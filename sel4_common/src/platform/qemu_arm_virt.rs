use crate::arch::config::KDEV_BASE;
use crate::sel4_config::UINT64_MAX;
use aarch64_cpu::registers::{
    Readable, Writeable, CNTFRQ_EL0, CNTVCT_EL0, CNTV_CTL_EL0, CNTV_CVAL_EL0, CNTV_TVAL_EL0,
};
use core::ptr::NonNull;
use serial_frame::SerialDriver;
use serial_impl_pl011::Pl011Uart;

use super::{
    time_def::{ticks_t, TIMER_CLOCK_HZ},
    Timer_func,
};

pub(crate) const CONFIGURE_TIMER_FREQUENCY: usize = 62500000;
#[cfg(feature = "kernel_mcs")]
#[allow(dead_code)]
pub(crate) const CONFIGURE_CLK_MAGIC: usize = 4611686019;
#[cfg(feature = "kernel_mcs")]
#[allow(dead_code)]
pub(crate) const CONFIGURE_CLK_SHIFT: usize = 58;
#[cfg(feature = "kernel_mcs")]
#[allow(dead_code)]
pub(crate) const CONFIGURE_KERNEL_WCET: usize = 10;
#[cfg(feature = "kernel_mcs")]
#[allow(dead_code)]
pub(crate) const TIMER_PRECISION: usize = 0;
#[cfg(feature = "kernel_mcs")]
#[allow(dead_code)]
pub(crate) const TIMER_OVERHEAD_TICKS: usize = 0;

pub struct timer;

impl Timer_func for timer {
    fn init_timer(self) {
        // Here use the generic timer init
        // check frequency is correct
        let gpt_cntfrq = CNTFRQ_EL0.get() as usize;
        if gpt_cntfrq != 0 && gpt_cntfrq != TIMER_CLOCK_HZ {
            panic!("The gpt_cntfrq is unequal to the system configure");
        }
        #[cfg(feature = "kernel_mcs")]
        {
            self.ack_deadline_irq();
            CNTV_CTL_EL0.set(bit!(0) as u64);
        }
        #[cfg(not(feature = "kernel_mcs"))]
        {
            self.reset_timer();
        }
    }
    fn get_current_time(self) -> ticks_t {
        CNTVCT_EL0.get() as _
    }
    fn set_deadline(self, deadline: ticks_t) {
        CNTV_CVAL_EL0.set(deadline as u64);
    }
    /// Reset the current Timer
    #[no_mangle]
    fn reset_timer(self) {
        /*
            SYSTEM_WRITE_WORD(CNT_TVAL, TIMER_RELOAD);
            SYSTEM_WRITE_WORD(CNT_CTL, BIT(0));
        */
        // TODO: Set a proper timer clock
        CNTV_TVAL_EL0.set(TIMER_CLOCK_HZ as u64 / 1000 * 2);
        CNTV_CTL_EL0.set(1);
    }
    fn ack_deadline_irq(self) {
        let deadline: ticks_t = UINT64_MAX;
        self.set_deadline(deadline);
    }
}

pub fn default_serial() -> impl SerialDriver {
    Pl011Uart::new(NonNull::new(KDEV_BASE as _).unwrap())
}
