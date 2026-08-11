// GIC register blocks: platform-specific virtual addresses.
// BCM2711: GIC is a single contiguous block; GICH = GIC_BASE + 0x4000.
// QEMU:    GIC components are separate devices; GICH = separate mapping.
use sel4_common::arch::config::KDEV_BASE;

pub const GIC_V2_DISTRIBUTOR_PPTR: usize = KDEV_BASE + 0x1000;
pub const GIC_V2_CONTROLLER_PPTR: usize = KDEV_BASE + 0x2000;
#[cfg(feature = "platform_bcm2711")]
pub const GIC_V2_VCPUIFACE_PPTR: usize = KDEV_BASE + 0x4000;
#[cfg(not(feature = "platform_bcm2711"))]
pub const GIC_V2_VCPUIFACE_PPTR: usize = KDEV_BASE + 0x3000;

/// Number of VGIC list registers supported (GICv2 maximum).
pub const GIC_V2_VCPU_MAX_LR: usize = 16;

/// Interrupt number for VGIC maintenance.
/// This is a PPI (Private Peripheral Interrupt) ID 25.
pub const INTERRUPT_VGIC_MAINTENANCE: usize = 25;

pub const IRQ_SET_ALL: u32 = 0xffffffff;
pub const IRQ_MASK: u32 = (1 << (10)) - 1;
pub const IRQ_NONE: u32 = 1023;