pub const CONFIGURE_TIMER_FREQUENCY: usize = 10000000;
use super::Timer_func;
use crate::arch::config::RESET_CYCLES;
use crate::arch::{get_time, set_timer};
use crate::platform::time_def::ticks_t;
use core::arch::asm;
use core::ptr::NonNull;
use serial_frame::SerialDriver;
use serial_impl_sbi::SerialSBI;

pub struct timer;

impl Timer_func for timer {
    fn init_timer(self) {}
    fn get_current_time(self) -> ticks_t {
        get_time()
    }
    fn set_deadline(self, deadline: ticks_t) {
        set_timer(deadline)
    }
    #[no_mangle]
    fn reset_timer(self) {
        let mut target = read_time() + RESET_CYCLES;
        set_timer(target);
        while read_time() > target {
            target = read_time() + RESET_CYCLES;
            set_timer(target);
        }
    }
    fn ack_deadline_irq(self) {}
}

pub fn read_time() -> usize {
    let temp: usize;
    unsafe {
        asm!("rdtime {}",out(reg)temp);
    }
    temp
}

/// Initialize Default Serial Driver
pub fn default_serial() -> impl SerialDriver {
    // 0xf is a random number, the argument of this function will never be used
    SerialSBI::new(unsafe { NonNull::new_unchecked(0xf as _) })
}
