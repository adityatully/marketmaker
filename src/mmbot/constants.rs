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

// time gap after which quoting will happen 
pub const QUOTING_GAP : Duration = Duration::from_millis(200);

// the entire incremental requote and should requote logic 
pub const MANAGEMENT_CYCLE_GAP : Duration = Duration::from_millis(250);

// inventory managment target 
pub const TARGET_INVENTORY : Decimal = dec!(0); 

// maximim size for an order 
pub const MAX_SIZE_FOR_ORDER : Decimal = dec!(100) ; 

// max inventory limit
pub const INVENTORY_CAP :Decimal = dec!(1000);


pub const MAX_BOOK_MULT : Decimal = dec!(2);

// inventory percentage to cancel 
pub const INVENTORY_CANCELLATION_TRIGGER_AMNT : Decimal = dec!(0.85);

// max age -> 15 mins
pub const MAX_ORDER_AGE: Duration = Duration::from_secs(15*60); 

// PNL CAPS
pub const MAX_ALLOWED_NEG_TOTAL_PNL : Decimal = dec!(-4000);
pub const MAX_ALLOWED_NEG_REALISED_PNL : Decimal = dec!(-2000);


// CANCEL THRESHOLDS FOR VAUROUS MODES 
// boostrap
pub const MAX_DISTANCE_IN_TICKS_TO_CANCEL_BOOTSTRAP : Decimal = dec!(20);
pub const MIN_PROFITABLE_SPREAD_IN_TICKS_BOOTSTRAP : Decimal = dec!(8);

// nomral 
pub const MAX_DISTANCE_IN_TICKS_TO_CANCEL_NORMAL : Decimal = dec!(10);
pub const MIN_PROFITABLE_SPREAD_IN_TICKS_NORMAL : Decimal = dec!(2);


pub const MAX_DISTANCE_IN_TICKS_TO_CANCEL_STRESSED : Decimal = dec!(7);
pub const MIN_PROFITABLE_SPREAD_IN_TICKS_STRESSED : Decimal = dec!(5);




// cancel threshhold , mid price , above this cancel 
pub const MAX_DISTANCE_IN_TICKS_TO_CANCEL : Decimal = dec!(12);

// min profitable spread in ticks , below this , cancel 
pub const MIN_PROFITABLE_SPREAD_IN_TICKS : Decimal = dec!(2);



// exit bootstrap 
pub const MIN_TRADES_TO_EXIT_BOOTSTRAP : u64 = 7;
pub const MIN_VOLUME_TO_EXIT_BOOTSTRAP : u64 = 400;
pub const MIN_SAMPLES_TO_EXIT_BOOTSTRAP : usize = 100;






pub const BOOTSTRAP_SPREAD_PCT : Decimal = dec!(0.05);
pub const BOOTSTRAP_LEVELS : usize = 4;
pub const NORMAL_LEVELS : usize = 6;
pub const NORMAL_SIZE_DECAY : f64 = 0.85;
pub const STRESSED_SPREAD_MULT : Decimal = dec!(2.5);
pub const STRESSED_LEVELS : usize = 4;
pub const CAPPED_LEVELS : usize = 2;


pub const BASE_SIZE_BOOTSTRAP: u64 = 100; // configure acc to the shares that the mm will be alloted after the ipo