#![no_std]
#![deny(warnings)]

#[cfg(target_arch = "aarch64")]
pub mod aarch64;

#[cfg(target_arch = "riscv64")]
pub mod riscv64;

pub mod regs;
pub mod tcb;
