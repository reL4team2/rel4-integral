pub const PPTR_TOP: usize = 0xFFFF_FFFF_8000_0000;
pub const PPTR_BASE: usize = 0xFFFF_FFC0_0000_0000;
pub const KDEV_BASE: usize = 0xFFFF_FFFF_C000_0000;

pub const PADDR_BASE: usize = 0x0;
pub const PADDR_TOP: usize = PPTR_TOP - PPTR_BASE_OFFSET;
pub const PPTR_BASE_OFFSET: usize = PPTR_BASE - PADDR_BASE;
