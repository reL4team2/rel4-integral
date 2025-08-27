// pub const GIC_V2_PPTR: usize = 0x8000000;
#[cfg(not(feature = "hypervisor"))]
pub const GIC_V2_PPTR: usize = 0xffffffffffe00000;
#[cfg(feature = "hypervisor")]
pub const GIC_V2_PPTR: usize = 0x000000ffffe00000;
pub const GIC_V2_DISTRIBUTOR_PPTR: usize = GIC_V2_PPTR + 0x1000;
pub const GIC_V2_CONTROLLER_PPTR: usize = GIC_V2_PPTR + 0x2000;

pub const IRQ_SET_ALL: u32 = 0xffffffff;
pub const IRQ_MASK: u32 = (1 << (10)) - 1;
pub const IRQ_NONE: u32 = 1023;
