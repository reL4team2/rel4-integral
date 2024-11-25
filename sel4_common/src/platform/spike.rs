pub const CONFIGURE_TIMER_FREQUENCY: usize = 10000000;
use super::Timer_func;
use crate::arch::config::RESET_CYCLES;
use crate::arch::{get_time, set_timer};
use crate::platform::time_def::ticks_t;
use core::arch::asm;

pub struct timer;

impl Timer_func for timer {
    fn initTimer(self) {}
    fn getCurrentTime(self) -> ticks_t {
        get_time()
    }
    fn setDeadline(self, deadline: ticks_t) {
        set_timer(deadline)
    }
    #[no_mangle]
    fn resetTimer(self) {
        let mut target = read_time() + RESET_CYCLES;
        set_timer(target);
        while read_time() > target {
            target = read_time() + RESET_CYCLES;
            set_timer(target);
        }
    }
    fn ackDeadlineIRQ(self) {}
}

pub fn read_time() -> usize {
    let temp: usize;
    unsafe {
        asm!("rdtime {}",out(reg)temp);
    }
    temp
}
