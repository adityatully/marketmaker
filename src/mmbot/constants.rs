use std::time::Duration;

use market_maker_rs::dec;
use rust_decimal::Decimal;


// constant for the global tick size 
pub const TICK_SIZE : Decimal = dec!(0.25);

// sampling gap , pushing the curr mid price into the rolling price array 
pub const SAMPLE_GAP : Duration = Duration::from_millis(50);

// max symbol 
pub const MAX_SYMBOLS : usize = 100;

// time gap after which volatility will be recalculated 
pub const VOLITILTY_CALC_GAP  : Duration = Duration::from_millis(100);

// the entire incremental requote and should requote logic 
pub const MANAGEMENT_CYCLE_GAP : Duration = Duration::from_millis(250);

// inventory managment target 
pub const TARGET_INVENTORY : Decimal = dec!(0); 

// maximim size for an order 
pub const MAX_SIZE_FOR_ORDER : Decimal = dec!(100) ; 

// max inventory limit , 
pub const INVENTORY_CAP :Decimal = dec!(1000);


pub const MAX_BOOK_MULT : Decimal = dec!(2);

// max age -> 15 mins
pub const MAX_ORDER_AGE: Duration = Duration::from_secs(15*60); 

// PNL CAPS
pub const MAX_ALLOWED_NEG_TOTAL_PNL : Decimal = dec!(-50000);
pub const MAX_ALLOWED_NEG_REALISED_PNL : Decimal = dec!(-25000);

pub const BASE_SIZE_BOOTSTRAP: u64 = 100; // configure acc to the shares that the mm will be alloted after the ipo

pub const WARMUP_DURATION : Duration = Duration::from_secs(400); 

pub const PRICE_TOLERANCE_IN_TICKS: Decimal = dec!(1);



