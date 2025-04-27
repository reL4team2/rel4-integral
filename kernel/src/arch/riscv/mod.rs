mod boot;
mod c_traps;
mod exception;
mod platform;

pub mod fpu;
pub use boot::try_init_kernel;
pub use c_traps::{restore_user_context, fastpath_restore};
use core::arch::asm;
pub use platform::{init_cpu, init_freemem};

pub use exception::handleUnknownSyscall;

#[cfg(feature = "ENABLE_SMP")]
pub use boot::try_init_kernel_secondary_core;

pub fn read_stval() -> usize {
    let temp: usize;
    unsafe {
        asm!("csrr {}, stval",out(reg)temp);
    }
    temp
}

pub fn read_sip() -> usize {
    let temp: usize;
    unsafe {
        asm!("csrr {}, sip",out(reg)temp);
    }
    temp
}

pub fn read_scause() -> usize {
    let temp: usize;
    unsafe {
        asm!("csrr {}, scause",out(reg)temp);
    }
    temp
}
