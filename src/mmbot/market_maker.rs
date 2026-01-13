use market_maker_rs::{Decimal, dec, market_state::volatility::VolatilityEstimator, prelude::{InventoryPosition, MarketState}};

use crate::shm::fill_queue_mm::MarketMakerFillQueue;
//use std::time::Instant;
const MAX_SYMBOLS : usize = 100;

pub struct SymbolState{
    pub inventory: InventoryPosition,          // current inventory 
    pub risk_aversion: Decimal,       
    pub time_to_terminal : u64,        
    pub liquidity_k: Decimal,            // order intensity 
    pub market_state : MarketState
    // need an array of prices to calculate volitiltiy , can use a vecdequeue , we only need an immutable ref to find volitiltiy 
}

// each symbol state shud have a defualt inventory for init 
impl SymbolState{
    pub fn new()->Self{
        Self { 
            inventory: InventoryPosition::new(),
            risk_aversion: dec!(1), 
            time_to_terminal: 0, 
            liquidity_k: dec!(0), 
            market_state: MarketState::new(dec!(0), dec!(0), 0) 
        }
    }
}

pub struct MarketMaker{
    pub fill_queue : MarketMakerFillQueue,
    pub symbol_detials : [SymbolState ; MAX_SYMBOLS],
    pub volitality_estimator : VolatilityEstimator
}

impl MarketMaker{
    pub fn new()->Self{
        let fill_queue = MarketMakerFillQueue::open("/tmp/MarketMakerFills");
        if fill_queue.is_err(){
            eprintln!("failed to open the fill queue");
        }
        Self { 
            fill_queue : fill_queue.unwrap(),
            symbol_detials: std::array::from_fn(|_| SymbolState::new()), 
            volitality_estimator: VolatilityEstimator::new()
        }
    }
}

