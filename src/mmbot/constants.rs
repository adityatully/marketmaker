use std::time::Duration;

use market_maker_rs::dec;
use rust_decimal::Decimal;


pub const TICK_SIZE : Decimal = dec!(0.25);

pub const SAMPLE_GAP : Duration = Duration::from_millis(50);

pub const MAX_SYMBOLS : usize = 100;

pub const VOLITILTY_CALC_GAP  : Duration = Duration::from_millis(100);

pub const QUOTING_GAP : Duration = Duration::from_millis(200);

pub const MANAGEMENT_CYCLE_GAP : Duration = Duration::from_millis(250);

pub const TARGET_INVENTORY : Decimal = dec!(0); 

pub const MAX_SIZE_FOR_ORDER : Decimal = dec!(100) ; 

pub const INVENTORY_CAP :Decimal = dec!(1000);

pub const MAX_BOOK_MULT : Decimal = dec!(2);

pub const MAX_DISTANCE_IN_TICKS_TO_CANCEL : Decimal = dec!(12);

pub const MIN_PROFITABLE_SPREAD_IN_TICKS : Decimal = dec!(2);

pub const INVENTORY_CANCELLATION_TRIGGER_AMNT : Decimal = dec!(0.85);

pub const MAX_ORDER_AGE: Duration = Duration::from_secs(15*60); 

pub const MAX_ALLOWED_NEG_TOTAL_PNL : Decimal = dec!(-4000);

pub const MAX_ALLOWED_NEG_REALISED_PNL : Decimal = dec!(-2000);