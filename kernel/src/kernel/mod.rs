pub mod boot;
pub mod fastpath;
#[cfg(target_arch = "riscv64")]
core::arch::global_asm!(include_str!("fastpath_restore.S"));
