pub mod consts;
pub mod gic_v2;

use core::ptr::NonNull;
pub use gic_v2::{dist_pending_clr, irq_disable, irq_enable, irq_is_edge_triggered, ipi_send_target};
use tock_registers::register_structs;
use tock_registers::registers::{ReadOnly, ReadWrite, WriteOnly};

register_structs! {
    /// GIC Distributor registers.
    #[allow(non_snake_case)]
    pub GicDistMapRegs{
        (0x000 => enable:ReadWrite<u32>),
        (0x0004 => ic_type: ReadOnly<u32>),
        (0x0008 => dist_ident: ReadOnly<u32>),
        (0x000c => _reserved_1),
        (0x0080 => security: [ReadWrite<u32>; 0x20]),
        (0x0100 => enable_set: [ReadWrite<u32>; 0x20]),
        (0x0180 => enable_clr: [ReadWrite<u32>; 0x20]),
        (0x0200 => pending_set: [ReadWrite<u32>; 0x20]),
        (0x0280 => pending_clr: [ReadWrite<u32>; 0x20]),
        (0x0300 => active: [ReadWrite<u32>; 0x20]),
        (0x0380 => res2: [ReadWrite<u32>; 0x20]),
        (0x0400 => priority: [ReadWrite<u32>; 0xff]),
        (0x07fC => _reserved_3),
        (0x0800 => targets: [ReadWrite<u32>; 0xff]),
        (0x0bfc => _reserved_4),
        (0x0c00 => config: [ReadWrite<u32>; 0x40]),
        (0x0d00 => spi: [ReadWrite<u32>; 0x20]),
        (0x0d80 => _reserved_5),
        (0x0dd4 => legacy_int: ReadWrite<u32>),
        (0x0dd8 => _reserved_7),
        (0x0de0 => match_d: ReadWrite<u32>),
        (0x0de4 => enable_d: ReadWrite<u32>),
        (0x0de8 => _reserved_8),
        (0x0f00 => sgi_control: WriteOnly<u32>),
        (0x0f04 => _reserved_9),
        (0x0f10 => sgi_pending_clr: [ReadWrite<u32>; 0x4]),
        (0x0f20 => _reserved_10),
        (0x0fc0 => periph_id: [ReadWrite<u32>; 12]),
        (0x0ff0 => component_id: [ReadWrite<u32>; 0x4]),
        (0x1000 => @END),
    }
}

register_structs! {
    /// GIC CPU Interface registers.
    #[allow(non_snake_case)]
    pub GicCpuIfaceMapRegs {
        (0x0000 => icontrol: ReadWrite<u32>),
        (0x0004 => pri_msk_c: ReadWrite<u32>),
        (0x0008 => pb_c: ReadWrite<u32>),
        (0x000c => int_ack: ReadOnly<u32>),
        (0x0010 => eoi: WriteOnly<u32>),
        (0x0014 => run_priority: ReadOnly<u32>),
        (0x0018 => hi_pend: ReadOnly<u32>),
        (0x001c => ns_alias_bp_c: ReadWrite<u32>),

        (0x0020 => ns_alias_ack: ReadWrite<u32>),
        (0x0024 => ns_alias_eoi: ReadWrite<u32>),
        (0x0028 => ns_alias_hi_pend: ReadWrite<u32>),
        (0x002c => _reserved_1),

        (0x0040 => integ_en_c: ReadWrite<u32>),
        (0x0044 => interrupt_out: ReadWrite<u32>),
        (0x0048 => _reserved_2),

        (0x0050 => match_c: ReadWrite<u32>),
        (0x0054 => enable_c: ReadWrite<u32>),
        (0x0058 => _reserved_3),

        (0x00D0 => active_priority: [ReadWrite<u32>; 0x4]),
        (0x00E0 => ns_active_priority: [ReadWrite<u32>; 0x4]),

        (0x00f0 => _reserved_4),

        (0x00fc => cpu_if_ident: ReadOnly<u32>),
        (0x0100 => _reserved_5),

        (0x0fc0 => periph_id: [ReadWrite<u32>; 0x8]),	//PL390 only
        (0x0fe0 => _reserved_6),
        (0x0ff0 => component_id: [ReadWrite<u32>; 0x4]),	//PL390 only
        /// Deactivate Interrupt Register.
        (0x1000 => @END),
    }
}

pub struct GicDistMap {
    base: NonNull<GicDistMapRegs>,
}

pub struct GicCpuIfaceMap {
    base: NonNull<GicCpuIfaceMapRegs>,
}

unsafe impl Send for GicDistMap {}
unsafe impl Sync for GicDistMap {}

unsafe impl Send for GicCpuIfaceMap {}
unsafe impl Sync for GicCpuIfaceMap {}

impl GicDistMap {
    /// Construct a new GIC distributor instance from the base address.
    pub const fn new(base: *mut u8) -> Self {
        Self {
            base: NonNull::new(base).unwrap().cast(),
        }
    }

    pub const fn regs(&self) -> &GicDistMapRegs {
        unsafe { self.base.as_ref() }
    }
}

impl GicCpuIfaceMap {
    /// Construct a new GIC CPU interface instance from the base address.
    pub const fn new(base: *mut u8) -> Self {
        Self {
            base: NonNull::new(base).unwrap().cast(),
        }
    }

    pub const fn regs(&self) -> &GicCpuIfaceMapRegs {
        unsafe { self.base.as_ref() }
    }
}
