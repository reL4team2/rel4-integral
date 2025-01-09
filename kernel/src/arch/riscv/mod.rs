mod boot;
mod c_traps;
mod exception;
mod platform;

pub use boot::try_init_kernel;
pub use c_traps::restore_user_context;
use core::arch::asm;
pub use platform::{init_cpu, init_freemem};

pub use exception::handleUnknownSyscall;

core::arch::global_asm!(include_str!("restore_fp.S"));
#[cfg(feature = "BUILD_BINARY")]
core::arch::global_asm!(include_str!("head.S"));
#[cfg(feature = "BUILD_BINARY")]
core::arch::global_asm!(include_str!("traps.S"));

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
