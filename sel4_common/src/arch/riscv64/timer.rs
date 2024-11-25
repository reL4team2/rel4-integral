#[cfg(feature = "KERNEL_MCS")]
use crate::{
    platform::time_def::{
        ticks_t, time_t, KHZ_IN_MHZ, MS_IN_S, TIMER_CLOCK_HZ, TIMER_CLOCK_KHZ, TIMER_CLOCK_MHZ,
        USE_KHZ, US_IN_MS,
    },
    sel4_config::UINT64_MAX,
};
#[cfg(feature = "KERNEL_MCS")]
pub const TICKS_IN_US: usize = (TIMER_CLOCK_HZ / (US_IN_MS * MS_IN_S));
#[cfg(feature = "KERNEL_MCS")]
pub fn getKernelWcetUs() -> time_t {
    10
}
#[cfg(feature = "KERNEL_MCS")]
pub fn usToTicks(us: time_t) -> ticks_t {
    us * TICKS_IN_US
}
#[cfg(feature = "KERNEL_MCS")]
pub fn getTimerPrecision() -> ticks_t {
    usToTicks(1)
}
#[cfg(feature = "KERNEL_MCS")]
pub fn ticksToUs(ticks: ticks_t) -> time_t {
    ticks / (TICKS_IN_US as u32 as usize)
}
#[cfg(feature = "KERNEL_MCS")]
pub fn getMaxTicksToUs() -> ticks_t {
    UINT64_MAX
}
#[cfg(feature = "KERNEL_MCS")]
pub fn getMaxUsToTicks() -> time_t {
    UINT64_MAX / TICKS_IN_US
}
