use crate::platform::time_def::{MS_IN_S, TIMER_CLOCK_HZ};
use crate::MASK;

// boot 相关的常数
pub const PPTR_TOP: usize = 0xFFFFFFFF80000000;
pub const PHYS_BASE: usize = 0x80000000;
pub const KERNEL_ELF_PADDR_BASE: usize = PHYS_BASE + 0x4000000;
pub const KERNEL_ELF_BASE: usize = PPTR_TOP + (KERNEL_ELF_PADDR_BASE & MASK!(30));
pub const KERNEL_ELF_BASE_OFFSET: usize = KERNEL_ELF_BASE - KERNEL_ELF_PADDR_BASE;
pub const PPTR_BASE: usize = 0xFFFFFFC000000000;
pub const PADDR_BASE: usize = 0x0;
pub const PPTR_BASE_OFFSET: usize = PPTR_BASE - PADDR_BASE;
pub const PADDR_TOP: usize = PPTR_TOP - PPTR_BASE_OFFSET;
pub const RESET_CYCLES: usize = (TIMER_CLOCK_HZ / MS_IN_S) * 2;
pub const KDEV_BASE: usize = 0xFFFFFFFFC0000000;

pub const MAX_UNTYPED_BITS: usize = 38;

#[cfg(feature = "enable_smp")]
pub const IRQ_REMOTE_CALL_IPI: usize = crate::platform::INTERRUPT_IPI_0;
#[cfg(feature = "enable_smp")]
pub const IRQ_RESCHEDULE_IPI: usize = crate::platform::INTERRUPT_IPI_1;
