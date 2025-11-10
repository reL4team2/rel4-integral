/// kernel/include/arch/arm/arch/64/mode/hardware.h
// boot 相关的常数
pub use rel4_arch::aarch64::config::*;

// pub const PHYS_BASE: usize = 0x4000_0000;
pub const KERNEL_ELF_PADDR_BASE: usize = crate::platform::PHYS_BASE_RAW;
// pub const KERNEL_ELF_BASE: usize = PPTR_TOP + (KERNEL_ELF_PADDR_BASE & mask_bits!(30));
pub const KERNEL_ELF_BASE: usize = PPTR_BASE_OFFSET + KERNEL_ELF_PADDR_BASE;
pub const KERNEL_ELF_BASE_OFFSET: usize = KERNEL_ELF_BASE - KERNEL_ELF_PADDR_BASE;

#[cfg(feature = "enable_smp")]
pub const IRQ_REMOTE_CALL_IPI: usize = 0;
#[cfg(feature = "enable_smp")]
pub const IRQ_RESCHEDULE_IPI: usize = 1;

pub const MAX_UNTYPED_BITS: usize = 47;
