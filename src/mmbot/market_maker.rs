use market_maker_rs::{Decimal, dec, market_state::volatility::VolatilityEstimator, 
    prelude::{InventoryPosition, MarketState, PnL}, strategy::{avellaneda_stoikov::calculate_optimal_quotes}};
use rustc_hash::FxHashMap;
use std::{collections::VecDeque, time::{Instant}};
use crate::{mmbot::{constants::{PNL_MAX_LOSS, PNL_NO_LOSS, WARMUP_DURATION},
      rolling_price::RollingPrice, 
    types::{CancelData, InventorySatus, MmError, PnlRiskMultiplier, PostData, QuotingParamLimits, SafetyCheck, SymbolOrders, TargetLadder, TargetQuotes, TradingRegime}}, 
    shm::{feed_queue_mm::{MarketMakerFeed, MarketMakerFeedQueue}, 
    fill_queue_mm::{MarketMakerFill, MarketMakerFillQueue}, 
    order_queue_mm::{MarketMakerOrderQueue, MmOrder, QueueError}, 
    response_queue_mm::{MessageFromApi, MessageFromApiQueue}}};
use rust_decimal::prelude::ToPrimitive;
use crate::mmbot::types::{OrderState  , Side , PendingOrder};
use crate::mmbot::constants::{SAMPLE_GAP , MAX_SYMBOLS , VOLITILTY_CALC_GAP , 
    QUOTING_GAP , MANAGEMENT_CYCLE_GAP , TARGET_INVENTORY , MAX_SIZE_FOR_ORDER , INVENTORY_CAP , MAX_BOOK_MULT , 
    TICK_SIZE    ,
    MAX_ORDER_AGE 
}; 



#[derive(Debug)]
pub struct SymbolState{
    // inputs 
    pub symbol : u32,
    pub ipo_price: Decimal,

    // Market data
    pub best_bid: Decimal,
    pub best_ask: Decimal,
    pub best_bid_qty: u32,
    pub best_ask_qty: u32,

    // prev market data 
    pub prev_best_bid: Decimal,
    pub prev_best_ask: Decimal,
    pub prev_best_bid_qty: u32,
    pub prev_best_ask_qty: u32,
    pub prev_mid_price: Decimal,
    // market state for storing volatility and mid price for each symbol 
    pub market_state : MarketState,
    // rolling price history for volatility calculation , each symbol 
    pub rolling_prices: RollingPrice, 
    // Inventory for each symbol long , short how much 
    pub inventory : InventoryPosition,
    pub pnl       : PnL,
    // Timing
    pub last_volatility_calc: Instant, // last volatility calculation
    pub last_sample_time: Instant, // when did we add the mid price to the rolling prices array last 
    pub last_management_cycle_time : Instant,
    // AS model constants 
    pub risk_aversion: Decimal,       
    pub time_to_terminal : u64,        
    pub liquidity_k: Decimal,            // order intensity 
    // keeping model constants per symbol , an auto adjusting formula needs to be developed to modify these 
    // according to market conditions
    pub regime : TradingRegime,
    pub regime_start_time : Instant
}



// each symbol state shud have a defualt inventory for init (ik)
impl SymbolState{
    pub fn new(ipo_price : Decimal , symbol:u32)->Self{
        Self { 
            symbol ,
            ipo_price , 
            best_ask : ipo_price,
            best_bid : ipo_price ,
            best_ask_qty : 0 , 
            best_bid_qty : 0 ,
            prev_best_ask : dec!(0),
            prev_best_bid : dec!(0) ,
            prev_best_bid_qty : 0 , 
            prev_best_ask_qty : 0 ,
            prev_mid_price : ipo_price,
            market_state : MarketState { mid_price: ipo_price, volatility: dec!(0), timestamp: 0 } ,
            rolling_prices : RollingPrice { deque: VecDeque::with_capacity(100), capacity: 100 } ,
            inventory : InventoryPosition::new() ,
            pnl : PnL::new(),
            last_sample_time : Instant::now() ,
            last_volatility_calc : Instant::now() ,
            last_management_cycle_time : Instant::now(),
            risk_aversion :dec!(0) , // decide ,,
            time_to_terminal : 0 , // decide 
            liquidity_k : dec!(0) , // decide , 
            regime : TradingRegime::WarmUp , 
            regime_start_time : Instant::now()
        }
        // find the sollutiton for the best bid and the best ask value at cold start 
    }

    pub fn compute_quote_sizes(
        &self,
    ) -> (u64, u64) {
       
        if INVENTORY_CAP <= dec!(0) || MAX_SIZE_FOR_ORDER == dec!(0) {
            return (0, 0);
        }

        let inv = self.inventory.quantity;
        let deviation = inv - TARGET_INVENTORY; 
        let abs_dev = deviation.abs();
        let inv_ratio = (abs_dev / INVENTORY_CAP).min(dec!(1));
        let inv_ratio_f = inv_ratio.to_f64().unwrap_or(1.0);

        
        let vol = self.market_state.volatility.max(dec!(0));
        let vol_factor = dec!(1) / (dec!(1) + vol); // in (0,1] , harmonic decay 
        let vol_factor_f = vol_factor.to_f64().unwrap_or(0.1);

       
        let mut base = (MAX_SIZE_FOR_ORDER.to_f64().unwrap_or(100.0) * vol_factor_f).round() as i64;
        base = std::cmp::max(1 , base); 

        
        // 1-inventory ratio btw 0-1
        let risky_mult = (1.0 - inv_ratio_f).clamp(0.0, 1.0);

        // 1 + inv ratio , btw 1-2
        let safe_mult  = (1.0 + inv_ratio_f).clamp(1.0, 2.0);


      
        // deviation positive , we are long
        let (mut bid_size, mut ask_size) = if deviation >= dec!(0) {
            // we need to sell more  , reduce bid size  , increase sell size
            ((base as f64 * risky_mult).round() as i64,
             (base as f64 * safe_mult).round() as i64)
        } else {
           // deviation neg , we are shott 
           // we need to buy more 
            ((base as f64 * safe_mult).round() as i64,
             (base as f64 * risky_mult).round() as i64)
        };

        
        if abs_dev >= INVENTORY_CAP {
          
            if deviation > dec!(0) {
              
                bid_size = 0;
            } else if deviation < dec!(0) {
              
                ask_size = 0;
            } else {
               
            }
        }

      
        let max_size_i64 = MAX_SIZE_FOR_ORDER.to_i64().unwrap_or(50);
        bid_size = bid_size.clamp(0, max_size_i64);
        ask_size = ask_size.clamp(0, max_size_i64);

        let best_bid_qty = self.best_bid_qty as u64;
        let best_ask_qty = self.best_ask_qty as u64;
        let max_book_mult_u64 = MAX_BOOK_MULT.to_u64().unwrap_or(2);

        if best_bid_qty > 0 {
            let cap = best_bid_qty.saturating_mul(max_book_mult_u64).max(1);
            bid_size = bid_size.min(cap as i64);
        }
        if best_ask_qty > 0 {
            let cap = best_ask_qty.saturating_mul(max_book_mult_u64).max(1);
            ask_size = ask_size.min(cap as i64);
        }

        (bid_size as u64, ask_size as u64)
    }


    pub fn determine_regime(&mut self )->TradingRegime{

        if self.regime == TradingRegime::WarmUp{
            let enough_time = self.regime_start_time.elapsed() >= WARMUP_DURATION;
            let enough_samples = self.rolling_prices.len() >= 20 ;

            if enough_samples && enough_time{
                self.regime = TradingRegime::Normal;
                self.regime_start_time = Instant::now();
                return TradingRegime::Normal;
            }

            return TradingRegime::WarmUp;
        }

        TradingRegime::Normal
    }
    pub fn get_quoting_params(&self)->QuotingParamLimits{
        match self.regime {
            TradingRegime::Normal=>{
                QuotingParamLimits { 
                    num_levels: 5, 
                    should_use_as: true, 
                    min_spread_ticks: dec!(10),    // should reducde this 
                    max_distance_from_mid: dec!(10) 
                }
            }
            TradingRegime::WarmUp=>{
                QuotingParamLimits { 
                    num_levels: 3, 
                    should_use_as: false, 
                    min_spread_ticks: dec!(8), 
                    max_distance_from_mid: dec!(20) 
                }
            }
        }
    }

    pub fn compute_volatility_multiplier(&self) -> Decimal {
        let vol = self.market_state.volatility;
        // 1.0x at 0% vol → 3.0x at 20% vol
        dec!(1.0) + (vol * dec!(10.0)).min(dec!(2.0))
    }

    pub fn compute_pnl_risk_multiplier(&self) -> PnlRiskMultiplier {
        let total_loss = self.pnl.total.min(dec!(0)).abs();
        let realized_loss = self.pnl.realized.min(dec!(0)).abs();
        let worst_loss = total_loss.max(realized_loss);
        
        let loss_ratio = (worst_loss / PNL_MAX_LOSS).min(dec!(1.0));
        
        // PnL affects SIZE ONLY (spread is AS-optimal)
        PnlRiskMultiplier {
            spread_mult: dec!(1.0),  // NO spread adjustment
            size_mult: (dec!(1.0) - (loss_ratio * dec!(0.7))).max(dec!(0.3)),
        }
    }
}

pub struct SymbolContext{
    pub state :  SymbolState , 
    pub orders : SymbolOrders
}


impl SymbolContext{
    pub fn new(ipo_price : Decimal , symbol : u32)->Self{
        Self{
            state : SymbolState::new(ipo_price, symbol) , 
            orders : SymbolOrders::new(symbol)
        }
    }

    pub fn check_if_time_caused_cancellation( &mut self,
        symbol: u32, cancel_batch: &mut Vec<CancelData>, ){
        

        for order in &mut self.orders.pending_orders {
            if order.state != OrderState::Active {
                continue;
            }

            let age = order.created_at.elapsed();
            if age > MAX_ORDER_AGE {
                if let Some(order_id) = order.exchange_order_id {
                    // sen directly to the order cancell queue , expose a function 
                    cancel_batch.push(CancelData { symbol  , client_id: order.client_id, order_id: Some(order_id) });
                    order.state = OrderState::PendingCancel;
                }
            }
        }
    }
    pub fn should_requote(&self) -> bool {


        // dont quote again in emergency mode 
        //if matches!(self.state.current_mode, QuotingMode::Emergency) {
        //    return false;
        //}
        

        // not enough time passed 
        if self.orders.last_quote_time.elapsed() < QUOTING_GAP {
            return false;
        }

        // mode chNged 
        //if self.state.current_mode != self.state.prev_mode {
        //    return true;
        //}
        
        // getting active orders
        let active_bids = self.orders.pending_orders.iter()
            .filter(|o| o.side == Side::BID && matches!(o.state, OrderState::Active))
            .count();
        
        let active_asks = self.orders.pending_orders.iter()
            .filter(|o| o.side == Side::ASK && matches!(o.state, OrderState::Active))
            .count();
        
        let total_active = active_bids + active_asks;
        
        
        if total_active == 0 {
            return true;
        }
        
        


        let mid_move = (self.state.market_state.mid_price - self.state.prev_mid_price).abs();
        let mid_move_ticks = mid_move/TICK_SIZE;
        //let mid_move_pct = if self.state.prev_mid_price != dec!(0) {
        //    (mid_move / self.state.prev_mid_price).to_f64().unwrap_or(0.0)
        //} else {
        //    0.0
        //};

         // 0.05% mid drift triggers requote (tune this)
        if mid_move_ticks >= dec!(2) {
            return true;
        }
        // default dont 
        false
    }


    pub fn compute_target_ladder(&self)->Result<TargetLadder , MmError>{
        let mut bids = Vec::new();
        let mut asks = Vec::new();
        
        let mid = self.state.market_state.mid_price;
        let current_spread = self.state.best_ask - self.state.best_bid;
        let spread_ticks = current_spread / TICK_SIZE;
        // safety check
        if spread_ticks < dec!(1) {
            return Ok(TargetLadder {
                bids: Vec::new(),
                asks: Vec::new(),
            });
        }

        let (mut best_bid , mut best_ask) = match self.state.regime{
            TradingRegime::WarmUp =>{
                let spread = mid * dec!(0.02);
                (mid - spread / dec!(2), mid + spread / dec!(2))
            }

            TradingRegime::Normal=>{
                let quotes = calculate_optimal_quotes(
                    self.state.market_state.mid_price, 
                    self.state.inventory.quantity, 
                    self.state.risk_aversion, 
                    self.state.market_state.volatility, 
                    self.state.time_to_terminal, 
                    self.state.liquidity_k
                ).map_err(|_| MmError::ASquoteError)?;
                (quotes.0 , quotes.1)
            }
        };


        let quoting_limits = self.state.get_quoting_params();

        let curr_spread_ticks = (best_ask - best_ask)/TICK_SIZE;

        if curr_spread_ticks < quoting_limits.min_spread_ticks{
            let min_spread = quoting_limits.min_spread_ticks * TICK_SIZE;
            let mid_spread = min_spread / dec!(2);
            best_bid = mid - mid_spread;
            best_ask = mid + mid_spread;
        }
        
        
        // inv caps 
        let inv_abs = self.state.inventory.quantity.abs();
        let should_quote_bid = !(inv_abs >= INVENTORY_CAP && self.state.inventory.quantity > dec!(0));
        let should_quote_ask = !(inv_abs >= INVENTORY_CAP && self.state.inventory.quantity < dec!(0));


        let (base_bid_size , base_ask_size) = self.state.compute_quote_sizes();

        let pnl_risk = self.state.compute_pnl_risk_multiplier();
        let final_bid_size = (base_bid_size as f64 * pnl_risk.size_mult.to_f64().unwrap_or(1.0)) as u64;
        let final_ask_size = (base_ask_size as f64 * pnl_risk.size_mult.to_f64().unwrap_or(1.0)) as u64;

        let num_levels = quoting_limits.num_levels;

        if should_quote_bid && final_bid_size > 0 {
            for i in 0..num_levels {
                let offset = TICK_SIZE * Decimal::from(i);
                let size = (final_bid_size as f64 * 0.85_f64.powi(i as i32)).max(10.0) as u32;
                
                bids.push(TargetQuotes {
                    price: best_bid - offset,
                    qty: size,
                    side: Side::BID,
                    level: i,
                });
            }
        }
        
        // Build asks
        if should_quote_ask && final_ask_size > 0 {
            for i in 0..num_levels {
                let offset = TICK_SIZE * Decimal::from(i);
                let size = (final_ask_size as f64 * 0.85_f64.powi(i as i32)).max(10.0) as u32;
                
                asks.push(TargetQuotes {
                    price: best_ask + offset,
                    qty: size,
                    side: Side::ASK,
                    level: i,
                });
            }
        }
        
        Ok(TargetLadder { bids, asks })


    }




    pub fn incremental_requote(&mut self ,  target_ladder : &mut TargetLadder , symbol : u32)->Result<(Vec<(u64 , u64)> , Vec<PostData>) , MmError>{
        const PRICE_TOLERANCE: Decimal = dec!(0.1);  // 10 cent / 10 paise 
        
     //   let mut orders_to_keep = Vec::new();
        let mut order_to_cancel = Vec::new();   
        let mut order_to_post = Vec::new();



        for order in &mut self.orders.pending_orders{
            // if state other than these two , we can skip 
            if !matches!(order.state, OrderState::Active | OrderState::PartiallyFilled) {
                continue;
            }

            let should_keep = match order.side {
                Side::BID =>{
                    // this is a bid order , 
                    target_ladder.bids.iter().any(
                        |target_quote|
                        target_quote.level == order.level && (order.price - target_quote.price).abs() <= PRICE_TOLERANCE
                    )
                }
                Side::ASK=>{
                    target_ladder.asks.iter().any(
                        |target_quote|
                        target_quote.level == order.level && (order.price - target_quote.price).abs() <= PRICE_TOLERANCE
                    )
                }
            };

            if should_keep {
                //orders_to_keep.push(order);
                // if it is not in cancel we obviously are keeping it 
            }
            else{
                // we send for canncelation 
                if let Some(order_id) = order.exchange_order_id{
                    order_to_cancel.push((order_id , order.client_id)); // push here for now can cancel in main loop
                }
            }
        }
          // identifiying the levels which are required to be posted 
        for target_quote in &mut target_ladder.asks{
            let already_have = self.orders.pending_orders.iter().any(
                |current_quote|
                target_quote.side == current_quote.side
                 && target_quote.level == current_quote.level 
                 && (target_quote.price-current_quote.price).abs() <= PRICE_TOLERANCE
            );

            if !already_have {
                // we need to post this order , either we would return all the orders to be posted , or just post from here 
                order_to_post.push(PostData{
                    price : target_quote.price ,
                    qty : target_quote.qty , 
                    side : target_quote.side,
                    symbol ,
                    level : target_quote.level
                });
            }
        }

        for target_quote in &mut target_ladder.bids{
            let already_have = self.orders.pending_orders.iter().any(
                |current_quote|
                target_quote.side == current_quote.side
                 && target_quote.level == current_quote.level 
                 && (target_quote.price-current_quote.price).abs() <= PRICE_TOLERANCE
            );

            if !already_have {
                // we need to post this order , either we would return all the orders to be posted , or just post from here 
                order_to_post.push(PostData{
                    price : target_quote.price ,
                    qty : target_quote.qty , 
                    side : target_quote.side , 
                    symbol,
                    level : target_quote.level
                });
            }
        }

        self.orders.last_quote_time = Instant::now();

        Ok((order_to_cancel , order_to_post))
    }


    pub fn safety_cancel_check(&mut self , order : PendingOrder)->SafetyCheck{
            if order.state != OrderState::Active {
                return SafetyCheck::OrderNotActive;
            }
            let mut cancel = false;
            
            if order.created_at.elapsed() >= MAX_ORDER_AGE{
                cancel = true;
            }

            match order.side {
                Side::BID => if order.price >= self.state.best_ask { 
                    cancel = true;
                 },
                Side::ASK => if order.price <= self.state.best_bid { 
                    cancel = true; 
                }
            }

            if cancel{
                return SafetyCheck::Fail
            }

            SafetyCheck::Pass
       
    }
}

pub struct MarketMaker{

    // Data manager for all symbols 
  //  pub symbol_states: FxHashMap<u32, SymbolState>,
    pub message_queue : MessageFromApiQueue,
    pub fill_queue    : MarketMakerFillQueue,
    pub feed_queue    : MarketMakerFeedQueue,
    pub volitality_estimator : VolatilityEstimator ,



    //ORDER MANAGER 
    pub order_queue   : MarketMakerOrderQueue,
  //  pub symbol_orders: FxHashMap<u32, SymbolOrders>,


    pub symbol_ctx  : FxHashMap<u32 , SymbolContext>,

    // quoting engine , currrent mode shud also be per symbol 


    pub cancel_batch : Vec<CancelData>,
    pub post_bacth   : Vec<PostData>
  
}

impl MarketMaker{
    pub fn new()->Self{
        let fill_queue = MarketMakerFillQueue::open("/tmp/MarketMakerFills");
        if fill_queue.is_err(){
            eprintln!("failed to open the fill queue");
        }
        let feed_queue = MarketMakerFeedQueue::open("/tmp/MarketMakerFeed");
        if feed_queue.is_err(){
            eprint!("failed to open feed queue");
        }
        let order_queue = MarketMakerOrderQueue::open("/tmp/MarketMakerOrders");
        if order_queue.is_err(){
            eprint!("failed to open order queue");
        }
        let message_from_api_queueu = MessageFromApiQueue::open("/tmp/MessageFromApiToMM");
        if message_from_api_queueu.is_err(){
            eprint!("fai;ed to open message queue");
        }
        Self { 
            //symbol_orders : FxHashMap::with_capacity_and_hasher(MAX_SYMBOLS, Default::default()),
            order_queue : order_queue.unwrap(),
            fill_queue : fill_queue.unwrap(),
            feed_queue : feed_queue.unwrap(),
            message_queue : message_from_api_queueu.unwrap(),
            volitality_estimator: VolatilityEstimator::new() , 
            symbol_ctx : FxHashMap::with_capacity_and_hasher(MAX_SYMBOLS, Default::default()),
            //symbol_states : FxHashMap::with_capacity_and_hasher(MAX_SYMBOLS, Default::default())
            cancel_batch : Vec::with_capacity(4096),
            post_bacth : Vec::with_capacity(4096),
        }
    }
    #[inline(always)]
    pub fn update_state_from_feed(&mut self , market_feed : MarketMakerFeed)->Result<() , MmError>{
        let symbol = market_feed.symbol;
        
        match self.symbol_ctx.get_mut(&symbol) {
            Some(ctx)=>{
                // store the prev best
                ctx.state.prev_best_bid = ctx.state.best_bid;
                ctx.state.prev_best_ask = ctx.state.best_ask;
                ctx.state.prev_best_bid_qty = ctx.state.best_bid_qty;
                ctx.state.prev_best_ask_qty = ctx.state.best_ask_qty;
                ctx.state.prev_mid_price = ctx.state.market_state.mid_price;

                ctx.state.best_ask = Decimal::from(market_feed.best_ask);
                ctx.state.best_bid = Decimal::from(market_feed.best_bid);
                ctx.state.best_ask_qty = market_feed.best_ask_qty;
                ctx.state.best_bid_qty = market_feed.best_bid_qty;

                ctx.state.market_state.mid_price = (ctx.state.best_ask + ctx.state.best_bid)/dec!(2);
                // mid price changed so the unrelaised pnl aslo changes 
                if ctx.state.inventory.quantity != dec!(0){
                    let new_unrealised = (ctx.state.market_state.mid_price - ctx.state.inventory.avg_entry_price)*ctx.state.inventory.quantity;
                    ctx.state.pnl.update(ctx.state.pnl.realized, new_unrealised);
                }
                // bootstrappijg per symbol 
            }
            None =>{
                return Err(MmError::SymbolNotFound);
            }
        }   
        Ok(())
    }
    #[inline(always)]
    pub fn update_inventory_from_fill(&mut self , market_fill : MarketMakerFill)->Result<() , MmError>{
        let symbol = market_fill.symbol;
        let fill_qty = Decimal::from(market_fill.fill_quantity);
        let fill_price = Decimal::from(market_fill.fill_price);
        match market_fill.side_of_mm_order{
            0 =>{
                 // market maker order was a buy (bid order)
                
                match self.symbol_ctx.get_mut(&symbol){
                    Some(ctx)=>{
                        let old_qty = ctx.state.inventory.quantity;
                        let old_avg = ctx.state.inventory.avg_entry_price;
                        ctx.state.inventory.quantity += fill_qty;

                        if ctx.state.inventory.quantity > dec!(0) {
                            ctx.state.inventory.avg_entry_price = 
                                (old_qty * old_avg + fill_qty * fill_price) 
                                / ctx.state.inventory.quantity;
                        }
                        //if it was a buy we got more shares , so the realised PNL wont change bcs we dint sell any quantity 
                        // unrealised PNL will 
                        let new_realised = ctx.state.pnl.realized;
                        let new_unrealised = (ctx.state.market_state.mid_price - ctx.state.inventory.avg_entry_price)*ctx.state.inventory.quantity;


                        ctx.state.pnl.update(new_realised, new_unrealised);
                    }
                    None =>{
                        return Err(MmError::SymbolNotFound);
                    }
                }
            }
            1 =>{
                // we had a sell order , some qty of  our inventory got sold 
                // update PNL 
                match self.symbol_ctx.get_mut(&symbol){
                    Some(ctx)=>{
                       // let old_qty = symbol_state.inventory.quantity;
                        let old_realised = ctx.state.pnl.realized;
                     //   let old_unrealised = symbol_state.pnl.unrealized;

                        // actual PNL that took place from the bid ask spread 
                        let realized_pnl_from_sale = (fill_price - ctx.state.inventory.avg_entry_price) * fill_qty;

                        ctx.state.inventory.quantity -= fill_qty;

                        let new_realized = old_realised + realized_pnl_from_sale;

                        let new_unrealized = if ctx.state.inventory.quantity != dec!(0) {
                            (ctx.state.market_state.mid_price - ctx.state.inventory.avg_entry_price) * ctx.state.inventory.quantity
                        } else {
                            dec!(0)  // No position = no unrealized P&L
                        };

                        ctx.state.pnl.update(new_realized, new_unrealized);
                    
                    }
                    None=>{
                        return Err(MmError::SymbolNotFound);
                    }
                }
            }   
            _ =>{

            }
        }
        Ok(())
    }


    #[inline(always)]
    pub fn order_manager_update_after_fill(&mut self , market_fill : MarketMakerFill)->Result<() , MmError>{
        let symbol = market_fill.symbol; 
        match self.symbol_ctx.get_mut(&symbol){
            Some(ctx)=>{
                if let Some( mm_order) = ctx.orders.pending_orders.iter_mut().find(|pending_order| 
                    match pending_order.exchange_order_id{
                        Some(order_id)=>{
                            order_id == market_fill.order_id_mm_order
                        }
                        None =>{
                            false
                        }
                    }
                ){
                    mm_order.remaining_size = mm_order.remaining_size.saturating_sub(market_fill.fill_quantity);

                    if mm_order.remaining_size == 0{
                        mm_order.state = OrderState::CompletelyFilled
                    }else{
                        mm_order.state = OrderState::PartiallyFilled
                    }


                }
                // we can remove the orders which are fully matched 
                ctx.orders.pending_orders.retain(|order| order.remaining_size > 0 );
            }
            None=>{
                return Err(MmError::SymbolNotFound);
            }
        }
        Ok(())
    }

    pub fn get_inventory_status(&mut self , symbol : u32)->Result<InventorySatus , MmError>{
        match self.symbol_ctx.get_mut(&symbol){
            Some(ctx)=>{
                if ctx.state.inventory.quantity > dec!(0){
                    Ok(InventorySatus::Long)
                }
                else{
                    Ok(InventorySatus::Short)
                }
            }

            None=>{
                return Err(MmError::SymbolNotFound);
            }
        }
    }
    #[inline(always)]
    pub fn handle_order_acceptance_ack(&mut self  , api_response : MessageFromApi)->Result<() , MmError>{
        let symbol = api_response.symbol;
        match self.symbol_ctx.get_mut(&symbol){
            Some(ctx)=>{
                if let Some(order) = ctx.orders.pending_orders.iter_mut().find(
                    |pending_order|
                    pending_order.client_id == api_response.client_id

                ){
                    order.exchange_order_id = Some(api_response.order_id);
                    order.state = OrderState::Active;
                }
            }
            None=>{
                return Err(MmError::SymbolNotFound);
            }
        }
        Ok(())
    }
    #[inline(always)]
    pub fn handle_order_cancel_ack(&mut self, api_response : MessageFromApi)->Result<() , MmError>{
        let symbol = api_response.symbol;
        match self.symbol_ctx.get_mut(&symbol){
            Some(ctx)=>{
                // remove it now , 
                ctx.orders.pending_orders.retain(|order| order.exchange_order_id != Some(api_response.order_id) && order.state == OrderState::PendingCancel);
                
            }
            None=>{
                return Err(MmError::SymbolNotFound);
            }
        }
        Ok(())
    }

    pub fn send_cancel_request(&mut self , symbol : u32 , client_id : u64 , order_id : u64 )->Result<() , QueueError>{
        match self.order_queue.enqueue(MmOrder { 
            order_id, 
            client_id, 
            price: 0, 
            timestamp: 0, 
            shares_qty: 0, 
            symbol, 
            side: 2, 
            order_type: 1, 
            status: 4
        }){
            Ok(_)=>{

            }
            Err(queue_error)=>{
                eprintln!(" enqueue erro {:?}" , queue_error);
            }
        }
        Ok(())
    }

    pub fn send_post_request(&mut self , symbol : u32 , price : Decimal , qty: u32, side : Side)->Result<() , QueueError>{
        match self.symbol_ctx.get_mut(&symbol){
            Some(ctx)=>{
                match self.order_queue.enqueue(MmOrder { 
                    order_id : 0 , 
                    client_id : ctx.orders.alloc_client_id() , 
                    price : price.to_u64().unwrap(), 
                    timestamp: 0, 
                    shares_qty: qty, 
                    symbol, 
                    side: match side {
                        Side::ASK => 1 ,
                        Side::BID => 0 
                    }, 
                    order_type: 0, 
                    status: 0
                }){
                    Ok(_)=>{
        
                    }
                    Err(queue_error)=>{
                        eprintln!(" enqueue erro {:?}" , queue_error);
                    }
                }
            }
            None =>{

            }
        }
        
        Ok(())
    }

    pub fn get_active_order_cnt(&self , symbol : u32)->Result<(usize , usize) , MmError>{
        let ctx = match self.symbol_ctx.get(&symbol){
            Some(context) => context , 
            None => {
                return Err(MmError::SymbolNotFound);
            }
        };

        let bids = ctx.orders.pending_orders.iter().filter(
            |order|
            order.side == Side::BID && order.state == OrderState::Active
        ).count();

        let asks = ctx.orders.pending_orders.iter().filter(
            |order|
            order.side == Side::ASK && order.state == OrderState::Active
        ).count();

        Ok((bids , asks ))
    }

    pub fn cancel_all_orders(&mut self , _ : u32){

    }

    pub fn cancel_side(&mut self , _ : u32){

    }

    // market maker running looop 

    pub fn run_market_maker(&mut self){
        loop{
            // clear the two batches 
            self.cancel_batch.clear();
            self.post_bacth.clear();

            
            while let Ok(Some(fill)) = self.fill_queue.dequeue(){
                let _ = self.order_manager_update_after_fill(fill);

                let _= self.update_inventory_from_fill(fill);
            }

            while let Ok(Some(feed)) = self.feed_queue.dequeue(){
                match self.update_state_from_feed(feed){
                    Ok(_)=>{
                    }
                    Err(error)=>{
                        eprintln!(" feed update error {:?}" , error);
                    }
                }
            }

            while let Ok(Some(api_message)) = self.message_queue.dequeue(){
                let symbol = api_message.symbol;
                match api_message.message_type{
                    0 =>{
                        // adding thr symbol , directly adding the context 
                        self.symbol_ctx.insert(symbol, SymbolContext::new(Decimal::from(api_message.ipo_price), symbol));
                    }
                    1 =>{
                        // order accepted ack
                        self.handle_order_acceptance_ack(api_message).expect("coulndt handle the order acceptance ");
                    }
                    2=>{
                        // cancale ordr ack
                        self.handle_order_cancel_ack(api_message).expect("coundt handle the order cancellation ack ")
                    }
                    _=>{
                        eprintln!("uidentified message type ");
                    }
                }
            }


            // updating the steate loop
            for (symbol  , ctx) in self.symbol_ctx.iter_mut(){
                let deref_symbol = *symbol;
                if ctx.state.last_sample_time.elapsed() >= SAMPLE_GAP{
                    ctx.state.rolling_prices.push(ctx.state.market_state.mid_price);
                    ctx.state.last_sample_time = Instant::now();
                }

                if ctx.state.last_volatility_calc.elapsed() >= VOLITILTY_CALC_GAP{
                    let new_vol = self.volitality_estimator.calculate_simple(ctx.state.rolling_prices.as_slice_for_volatility());
                    if new_vol.is_err(){
                        eprint!("error in volatility calc");
                    }
                    ctx.state.market_state.volatility = new_vol.unwrap();
                    ctx.state.last_volatility_calc = Instant::now();
                }

                if ctx.state.last_management_cycle_time.elapsed() >= MANAGEMENT_CYCLE_GAP{
                    for active_order in &mut ctx.orders.pending_orders{
                       
                       
                    }


                    // this is very rare that this function wuld get ca;;ed , its just a cleanup function 
                    ctx.check_if_time_caused_cancellation(*symbol, &mut self.cancel_batch);
                    

                    // if !ctx.state.is_bootstrapped && ctx.state.should_exit_bootstrap() {
                        // ctx.state.is_bootstrapped = true;
                    // }

                   // let _ = ctx.state.determine_mode(); // no need to return , just update the mode 
                    // can return emergency or invetnory capped also 


                    if ctx.should_requote(){
                        // we compute target laders and try to modify them 
                        //match ctx.compute_target_ladder(){
                        //    Ok(mut target_ladder)=>{
                        //       if let Ok(requote_result) = ctx.incremental_requote(&mut target_ladder , *symbol){
                        //            let orders_to_cancel = requote_result.0;
                        //            let orders_to_post = requote_result.1;
//
                        //            for order in orders_to_cancel {
                        //                self.cancel_batch.push(CancelData { symbol : deref_symbol , client_id: order.1, order_id: Some(order.0) });
                        //            }
//
                        //            for order in orders_to_post{
                        //                self.post_bacth.push(PostData { price: order.price, qty: order.qty, side: order.side , symbol : *symbol , level : order.level });
                        //            }
                        //       }
                        //    }
//
                        //    Err(_)=>{
                        //        eprint!("error occpured in the compute target ladder function ")
                        //    }
                        //}
                    }


                    // shoudl i send requsts here 



                }
            }

            // or shud i send requet here 
            // cudnt call the function becuse it took a mutable refrence to entire self 
            for cancel_order in &mut  self.cancel_batch{
                match cancel_order.order_id{
                    Some(id)=>{
                        match self.order_queue.enqueue(MmOrder { 
                            order_id : id, 
                            client_id : cancel_order.client_id, 
                            price: 0, 
                            timestamp: 0, 
                            shares_qty: 0, 
                            symbol : cancel_order.symbol, 
                            side: 2, 
                            order_type: 1, 
                            status: 4
                        }){
                            Ok(_)=>{
                
                            }
                            Err(queue_error)=>{
                                eprintln!(" enqueue erro {:?}" , queue_error);
                            }
                        }
                    }
                    None =>{

                    }
                }
            }


            for post_order in &mut self.post_bacth{
                match self.symbol_ctx.get_mut(&post_order.symbol){
                    Some(ctx)=>{
                        let client_id =  ctx.orders.alloc_client_id();
                        match self.order_queue.enqueue(MmOrder { 
                            order_id : 0 , 
                            client_id  , 
                            price : post_order.price.to_u64().unwrap(), 
                            timestamp: 0, 
                            shares_qty: post_order.qty, 
                            symbol : post_order.symbol, 
                            side: match post_order.side {
                                Side::ASK => 1 ,
                                Side::BID => 0 
                            }, 
                            order_type: 0, 
                            status: 0
                        }){
                            Ok(_)=>{
                                // push it to the order manager 
                                ctx.orders.pending_orders.push(PendingOrder { 
                                    client_id, 
                                    exchange_order_id: None, 
                                    side: post_order.side, 
                                    price: post_order.price, 
                                    original_size: post_order.qty, 
                                    remaining_size: post_order.qty, 
                                    state: OrderState::PendingNew, 
                                    level: post_order.level, 
                                    created_at: Instant::now() 
                                });
                            }
                            Err(queue_error)=>{
                                eprintln!(" enqueue erro {:?}" , queue_error);
                            }
                        }
                    }
                    None =>{
        
                    }
                }
            } 
        }
    }
}



