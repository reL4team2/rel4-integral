#![no_std]
#![deny(warnings)]

#[macro_use]
extern crate rel4_utils;

#[macro_use]
pub mod utils;

#[cfg(target_arch = "aarch64")]
pub mod aarch64;

#[cfg(target_arch = "riscv64")]
pub mod riscv64;

pub mod basic;
pub mod regs;
pub mod tcb;
