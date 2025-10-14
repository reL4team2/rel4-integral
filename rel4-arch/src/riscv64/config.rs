pub const PPTR_TOP: usize = 0xFFFFFFFF80000000;
pub const PPTR_BASE: usize = 0xFFFFFFC000000000;
pub const KDEV_BASE: usize = 0xFFFFFFFFC0000000;

pub const PADDR_BASE: usize = 0x0;
pub const PADDR_TOP: usize = PPTR_TOP - PPTR_BASE_OFFSET;
pub const PPTR_BASE_OFFSET: usize = PPTR_BASE - PADDR_BASE;
