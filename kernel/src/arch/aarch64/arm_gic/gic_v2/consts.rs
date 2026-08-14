// GIC register blocks: platform-specific virtual addresses.
// GICH (VCPU interface control) is at GIC_BASE + 0x3000 on both BCM2711 and QEMU.
// BCM2711's arm_local_intc (0xff800000) is a separate BCM local interrupt controller,
// NOT the GIC virtual interface.
use sel4_common::arch::config::KDEV_BASE;

pub const GIC_V2_DISTRIBUTOR_PPTR: usize = KDEV_BASE + 0x1000;
pub const GIC_V2_CONTROLLER_PPTR: usize = KDEV_BASE + 0x2000;
pub const GIC_V2_VCPUIFACE_PPTR: usize = KDEV_BASE + 0x3000;

/// Number of VGIC list registers supported (GICv2 maximum).
pub const GIC_V2_VCPU_MAX_LR: usize = 16;

/// Interrupt number for VGIC maintenance.
/// This is a PPI (Private Peripheral Interrupt) ID 25.
pub const INTERRUPT_VGIC_MAINTENANCE: usize = 25;

pub const IRQ_SET_ALL: u32 = 0xffffffff;
pub const IRQ_MASK: u32 = (1 << (10)) - 1;
pub const IRQ_NONE: u32 = 1023;