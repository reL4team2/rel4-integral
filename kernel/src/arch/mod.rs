#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "aarch64")]
pub use aarch64::*;

#[cfg(target_arch = "riscv64")]
mod riscv;
#[cfg(target_arch = "riscv64")]
pub use riscv::*;

#[cfg(feature = "BUILD_BINARY")]
core::arch::global_asm!(include_str!(concat!(env!("OUT_DIR"), "/head.S")));
#[cfg(feature = "BUILD_BINARY")]
core::arch::global_asm!(include_str!(concat!(env!("OUT_DIR"),"/traps.S")));