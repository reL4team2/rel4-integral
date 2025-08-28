#![allow(unused)]
pub mod arch_tcb;
pub mod config;
mod message_info;
mod object;
mod registers;
mod timer;
mod vm_rights;
pub use arch_tcb::ArchTCB;
pub use message_info::*;
pub use object::*;
pub use registers::*;
pub use timer::*;
pub use vm_rights::*;

#[cfg(feature = "enable_smp")]
mod smp;
#[cfg(feature = "enable_smp")]
pub use smp::*;

pub use rel4_arch::aarch64::psci::shutdown;

pub fn get_time() -> usize {
    todo!("get_time")
}
