use super::CONFIGURE_TIMER_FREQUENCY;

/// 时钟ticks
pub type ticks_t = usize;
pub type time_t = usize;

pub const MS_IN_S: usize = 1000;
pub const US_IN_MS: usize = 1000;
pub const HZ_IN_KHZ: usize = 1000;
pub const KHZ_IN_MHZ: usize = 1000;
pub const HZ_IN_MHZ: usize = 1000000;

pub const TIMER_CLOCK_HZ: usize = CONFIGURE_TIMER_FREQUENCY;
#[allow(clippy::absurd_extreme_comparisons)]
pub const USE_KHZ: bool = TIMER_CLOCK_HZ % HZ_IN_MHZ > 0;
pub const TIMER_CLOCK_KHZ: usize = TIMER_CLOCK_HZ / HZ_IN_KHZ;
pub const TIMER_CLOCK_MHZ: usize = TIMER_CLOCK_HZ / HZ_IN_MHZ;
