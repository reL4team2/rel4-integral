use rel4_arch::basic::{PAddr, PPtr, PRegion, Region};
use sel4_common::sel4_config::*;
use sel4_common::structures::{exception_t, seL4_IPCBuffer};
use sel4_common::structures_gen::{cap, cap_null_cap};
use sel4_cspace::interface::cte_t;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct BootInfoHeader {
    pub id: usize,
    pub len: usize,
}

#[allow(non_camel_case_types)]
pub type seL4_SlotPos = usize;

#[repr(C)]
#[derive(Default, Debug, PartialEq, Clone, Copy)]
pub struct SlotRegion {
    pub start: seL4_SlotPos,
    pub end: seL4_SlotPos,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct UntypedDesc {
    pub paddr: PAddr,
    pub sizeBits: u8,
    pub isDevice: u8,
    pub padding: [u8; 6],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct BootInfo {
    pub extraLen: usize,
    pub nodeID: usize,
    pub numNodes: usize,
    pub numIOPTLevels: usize,
    pub ipcBuffer: *const seL4_IPCBuffer,
    pub empty: SlotRegion,
    pub sharedFrames: SlotRegion,
    pub userImageFrames: SlotRegion,
    pub userImagePaging: SlotRegion,
    pub ioSpaceCaps: SlotRegion,
    pub extraBIPages: SlotRegion,
    pub initThreadCNodeSizeBits: usize,
    pub initThreadDomain: usize,
    #[cfg(feature = "kernel_mcs")]
    pub schedcontrol: SlotRegion,
    pub untyped: SlotRegion,
    pub untypedList: [UntypedDesc; CONFIG_MAX_NUM_BOOTINFO_UNTYPED_CAPS],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ndks_boot_t {
    pub reserved: [PRegion; MAX_NUM_RESV_REG],
    pub resv_count: usize,
    pub freemem: [Region; MAX_NUM_FREEMEM_REG],
    pub bi_frame: *mut BootInfo,
    pub slot_pos_cur: seL4_SlotPos,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct rootserver_mem_t {
    pub cnode: usize,
    pub vspace: usize,
    pub asid_pool: usize,
    pub ipc_buf: usize,
    pub boot_info: usize,
    pub extra_bi: usize,
    pub tcb: usize,
    #[cfg(feature = "kernel_mcs")]
    pub sc: usize,
    pub paging: Region,
}

#[repr(C)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct create_frames_of_region_ret_t {
    pub region: SlotRegion,
    pub success: bool,
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct lookupCap_ret_t {
    pub status: exception_t,
    pub capability: cap,
}

impl Default for lookupCap_ret_t {
    fn default() -> Self {
        lookupCap_ret_t {
            status: exception_t::EXCEPTION_NONE,
            capability: cap_null_cap::new().unsplay(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct lookupCapAndSlot_ret_t {
    pub status: exception_t,
    pub capability: cap,
    pub slot: *mut cte_t,
}

impl Default for lookupCapAndSlot_ret_t {
    fn default() -> Self {
        lookupCapAndSlot_ret_t {
            status: exception_t::EXCEPTION_NONE,
            capability: cap_null_cap::new().unsplay(),
            slot: 0 as *mut cte_t,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct syscall_error_t {
    pub invalidArgumentNumber: usize,
    pub invalidCapNumber: usize,
    pub rangeErrorMin: usize,
    pub rangeErrorMax: usize,
    pub memoryLeft: usize,
    pub failedLookupWasSource: usize,
    pub _type: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct extra_caps_t {
    pub excaprefs: [PPtr; SEL4_MSG_MAX_EXTRA_CAPS],
}
