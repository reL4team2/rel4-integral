#![no_std]
#![allow(non_snake_case)]
#![allow(internal_features)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![feature(decl_macro)]
#![feature(core_intrinsics)]

#[macro_use]
extern crate rel4_utils;
#[macro_use]
extern crate rel4_arch;

pub mod arch;
mod asid;
mod boot;
// mod pte;
mod structures;
mod utils;

#[cfg(target_arch = "aarch64")]
pub use arch::aarch64::*;
#[cfg(target_arch = "riscv64")]
pub use arch::riscv64::*;
pub use arch::unmap_page;
pub use asid::*;
pub use boot::*;
// pub use pte::PTE;
pub use structures::*;
pub use utils::check_vp_alignment;
// pub use riscv::*;
