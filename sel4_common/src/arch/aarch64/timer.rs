#[cfg(feature = "kernel_mcs")]
use crate::platform::CONFIGURE_KERNEL_WCET;
#[cfg(feature = "kernel_mcs")]
use crate::{
    platform::{
        time_def::{
            ticks_t, time_t, KHZ_IN_MHZ, TIMER_CLOCK_HZ, TIMER_CLOCK_KHZ, TIMER_CLOCK_MHZ, USE_KHZ,
        },
        TIMER_OVERHEAD_TICKS, TIMER_PRECISION,
    },
    sel4_config::UINT64_MAX,
};
#[cfg(feature = "kernel_mcs")]
pub fn get_max_ticks_to_us() -> ticks_t {
    if USE_KHZ {
        UINT64_MAX / TIMER_CLOCK_KHZ
    } else {
        UINT64_MAX
    }
}
#[cfg(feature = "kernel_mcs")]
pub fn get_max_us_to_ticks() -> time_t {
    if USE_KHZ {
        UINT64_MAX / TIMER_CLOCK_KHZ
    } else {
        UINT64_MAX / TIMER_CLOCK_MHZ
    }
}
#[cfg(feature = "kernel_mcs")]
pub fn ticks_to_us(ticks: ticks_t) -> time_t {
    if USE_KHZ {
        (ticks * KHZ_IN_MHZ) / TIMER_CLOCK_KHZ
    } else {
        ticks / TIMER_CLOCK_MHZ
    }
}
#[cfg(feature = "kernel_mcs")]
pub fn us_to_ticks(us: time_t) -> ticks_t {
    if USE_KHZ {
        (us * TIMER_CLOCK_KHZ) / KHZ_IN_MHZ
    } else {
        us * TIMER_CLOCK_MHZ
    }
}
#[cfg(feature = "kernel_mcs")]
pub fn get_timer_precision() -> ticks_t {
    us_to_ticks(TIMER_PRECISION) + TIMER_OVERHEAD_TICKS
}
#[cfg(feature = "kernel_mcs")]
pub fn get_kernel_wcet_us() -> time_t {
    CONFIGURE_KERNEL_WCET
}
