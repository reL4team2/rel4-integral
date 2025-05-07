#[cfg(feature = "KERNEL_MCS")]
use crate::platform::CONFIGURE_KERNEL_WCET;
#[cfg(feature = "KERNEL_MCS")]
use crate::{
    platform::{
        time_def::{
            ticks_t, time_t, KHZ_IN_MHZ, TIMER_CLOCK_HZ, TIMER_CLOCK_KHZ, TIMER_CLOCK_MHZ, USE_KHZ,
        },
        TIMER_OVERHEAD_TICKS, TIMER_PRECISION,
    },
    sel4_config::UINT64_MAX,
};
#[cfg(feature = "KERNEL_MCS")]
pub fn getMaxTicksToUs() -> ticks_t {
    if USE_KHZ {
        UINT64_MAX / TIMER_CLOCK_KHZ
    } else {
        UINT64_MAX
    }
}
#[cfg(feature = "KERNEL_MCS")]
pub fn getMaxUsToTicks() -> time_t {
    if USE_KHZ {
        UINT64_MAX / TIMER_CLOCK_KHZ
    } else {
        UINT64_MAX / TIMER_CLOCK_MHZ
    }
}
#[cfg(feature = "KERNEL_MCS")]
pub fn ticksToUs(ticks: ticks_t) -> time_t {
    if USE_KHZ {
        (ticks * KHZ_IN_MHZ) / TIMER_CLOCK_KHZ
    } else {
        ticks / TIMER_CLOCK_MHZ
    }
}
#[cfg(feature = "KERNEL_MCS")]
pub fn usToTicks(us: time_t) -> ticks_t {
    if USE_KHZ {
        (us * TIMER_CLOCK_KHZ) / KHZ_IN_MHZ
    } else {
        us * TIMER_CLOCK_MHZ
    }
}
#[cfg(feature = "KERNEL_MCS")]
pub fn getTimerPrecision() -> ticks_t {
    usToTicks(TIMER_PRECISION) + TIMER_OVERHEAD_TICKS
}
#[cfg(feature = "KERNEL_MCS")]
pub fn get_kernel_wcet_us() -> time_t {
    CONFIGURE_KERNEL_WCET
}
