use std::time::Instant;

use rust_decimal::Decimal;

// level are basically price levels  how deep to quote 


#[derive(Debug, Clone , Copy , PartialEq)]
pub enum TradingRegime{
    WarmUp ,
    Normal ,
}

pub struct QuotingParamLimits{
    pub num_levels : usize , 
    pub should_use_as : bool , 
    pub min_spread_ticks   : Decimal , 
    pub max_distance_from_mid : Decimal ,
}




#[derive(Debug)]
pub struct SymbolOrders {
    pub symbol: u32,
    pub pending_orders: Vec<PendingOrder>,
    pub next_client_id: u64,
    pub last_quote_time: Instant,
}


impl SymbolOrders{
    pub fn new(symbol : u32)->Self{
        Self { 
            symbol, 
            pending_orders: Vec::new(), 
            next_client_id: 1, 
            last_quote_time: Instant::now() 
        }
    }

    pub fn alloc_client_id(&mut self) -> u64 {
        let id = self.next_client_id;
        self.next_client_id += 1;
        id
    }
}


#[derive(Debug, Clone, PartialEq , Copy)]
pub enum Side {
    BID = 0 ,
    ASK = 1 
}

#[derive(Debug, Clone, PartialEq , Copy)]
pub enum OrderState {
    PendingNew, // send but ack not reicved 
    Active,
    PendingCancel,
    PartiallyFilled,
    CompletelyFilled
}


#[derive(Debug, Clone , Copy)]
pub struct PendingOrder{
    pub client_id: u64,
    pub exchange_order_id: Option<u64>,
    pub side: Side,
    pub price: Decimal,
    pub original_size: u32,
    pub remaining_size: u32,
    pub state: OrderState,
    pub level: usize,  // Which level in the ladder (0-9)
    pub created_at : Instant ,
}



#[derive(Debug)]
pub enum MmError{
    SymbolNotFound ,
    ClienIdNotFound , 
    CouldNotCalculateQuotes , 
    ASquoteError
}


#[derive(Debug, Clone, PartialEq , Copy)]
pub enum InventorySatus{
    Long ,
    Short
}

pub enum ApiMessageType{
    AddSymbolMessage = 0 , 
    OrderAcceptedAck = 1 ,
    OrderCancelledAck = 2 ,
}


pub struct DepthUpdate{
    pub old_best_bid : Decimal ,
    pub old_best_ask : Decimal ,
    pub new_best_bid : Decimal ,
    pub new_best_ask : Decimal, 
}


#[derive(Debug , Clone, Copy)]
pub struct CancelData{
    pub symbol : u32 , 
    pub client_id : u64 , 
    pub order_id  : Option<u64> 
}


#[derive(Debug , Clone, Copy)]
pub struct PostData{
    pub symbol : u32 ,
    pub price : Decimal , 
    pub qty   : u32 , 
    pub side  : Side , 
    pub level : usize 
}




#[derive(Debug , Clone, Copy)]
pub struct TargetQuotes{
    pub level : usize , 
    pub side : Side ,
    pub price  :  Decimal , 
    pub qty : u32
}

pub struct TargetLadder {
    pub bids : Vec<TargetQuotes> , 
    pub asks : Vec<TargetQuotes>,
}


pub enum SafetyCheck{
    Pass , 
    Fail , 
    OrderNotActive
}

#[derive(Debug)]
pub struct PnlRiskMultiplier {
    pub spread_mult: Decimal,
    pub size_mult: Decimal,
}