pub mod time_def;
// 这里除了arch的区别，还有不同arch下的platform的区别，这是两回事，我们这边目前只涉及到关于platform的timer的部分

#[cfg(target_arch = "aarch64")]
pub mod qemu_arm_virt;
#[cfg(target_arch = "aarch64")]
pub use qemu_arm_virt::*;

#[cfg(target_arch = "riscv64")]
pub mod spike;
// pub mod qemu_riscv_virt;

#[cfg(target_arch = "riscv64")]
pub use spike::*;
use time_def::ticks_t;

include!(concat!(env!("OUT_DIR"), "/platform_gen.rs"));

pub trait Timer_func {
    fn init_timer(self);
    fn get_current_time(self) -> ticks_t;
    fn set_deadline(self, deadline: ticks_t);
    fn reset_timer(self);
    fn ack_deadline_irq(self);
}
