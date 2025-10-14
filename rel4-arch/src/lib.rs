//! rel4-arch is a crate contains arch-specific and platform-specific resources.
//!
//! The structure of this crate is:
//!
//! - [aarch64] aarch64 specific resources, include instructions. pagetable and register definations
//! - [riscv64] riscv64 specific resources, include instructions, pagetable and register definations
//! - [basic] the foundation of rel4, this contains structure that used frequently.
//! - [platform] platform specific resources, code for platform specific resource, eg: driver address, memory size
//!
#![no_std]
#![deny(warnings)]

#[macro_use]
extern crate rel4_utils;

#[macro_use]
pub mod utils;

#[macro_use]
pub mod macros;

#[cfg(any(target_arch = "aarch64", doc))]
pub mod aarch64;

#[cfg(any(target_arch = "riscv64", doc))]
pub mod riscv64;

pub mod basic;
pub mod platform;
pub mod regs;

// pub mod tcb;
