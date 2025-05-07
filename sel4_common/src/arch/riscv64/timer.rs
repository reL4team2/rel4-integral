#[cfg(feature = "kernel_mcs")]
use crate::{
    platform::time_def::{
        ticks_t, time_t, KHZ_IN_MHZ, MS_IN_S, TIMER_CLOCK_HZ, TIMER_CLOCK_KHZ, TIMER_CLOCK_MHZ,
        USE_KHZ, US_IN_MS,
    },
    sel4_config::UINT64_MAX,
};
#[cfg(feature = "kernel_mcs")]
pub const TICKS_IN_US: usize = (TIMER_CLOCK_HZ / (US_IN_MS * MS_IN_S));
#[cfg(feature = "kernel_mcs")]
pub fn get_kernel_wcet_us() -> time_t {
    10
}
#[cfg(feature = "kernel_mcs")]
pub fn us_to_ticks(us: time_t) -> ticks_t {
    us * TICKS_IN_US
}
#[cfg(feature = "kernel_mcs")]
pub fn get_timer_precision() -> ticks_t {
    us_to_ticks(1)
}
#[cfg(feature = "kernel_mcs")]
pub fn ticks_to_us(ticks: ticks_t) -> time_t {
    ticks / (TICKS_IN_US as u32 as usize)
}
#[cfg(feature = "kernel_mcs")]
pub fn get_max_ticks_to_us() -> ticks_t {
    UINT64_MAX
}
#[cfg(feature = "kernel_mcs")]
pub fn get_max_us_to_ticks() -> time_t {
    UINT64_MAX / TICKS_IN_US
}
