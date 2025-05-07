/// kernel/include/arch/arm/arch/64/mode/hardware.h
// boot 相关的常数
pub const PPTR_TOP: usize = 0xffffffffc0000000;
// pub const PHYS_BASE: usize = 0x4000_0000;
pub const KERNEL_ELF_PADDR_BASE: usize = crate::platform::PHYS_BASE_RAW;
// pub const KERNEL_ELF_BASE: usize = PPTR_TOP + (KERNEL_ELF_PADDR_BASE & MASK!(30));
pub const KERNEL_ELF_BASE: usize = PPTR_BASE_OFFSET + KERNEL_ELF_PADDR_BASE;
pub const KERNEL_ELF_BASE_OFFSET: usize = KERNEL_ELF_BASE - KERNEL_ELF_PADDR_BASE;
pub const PPTR_BASE: usize = 0xffffff8000000000;
pub const PADDR_BASE: usize = 0x0;
pub const PPTR_BASE_OFFSET: usize = PPTR_BASE - PADDR_BASE;
pub const PADDR_TOP: usize = PPTR_TOP - PPTR_BASE_OFFSET;
pub const KDEV_BASE: usize = 0xffffffffffe00000;

#[cfg(feature = "enable_smp")]
pub const IRQ_REMOTE_CALL_IPI: usize = 0;
#[cfg(feature = "enable_smp")]
pub const IRQ_RESCHEDULE_IPI: usize = 1;

pub const MAX_UNTYPED_BITS: usize = 47;
