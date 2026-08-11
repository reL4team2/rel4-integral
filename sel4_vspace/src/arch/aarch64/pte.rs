use crate::{arch::aarch64::machine::clean_by_va_pou, vm_attributes_t, PTE};

use super::{mair_types, UPT_LEVELS, VSPACE_INDEX_BITS};
use crate::lookupPTSlot_ret_t;
use rel4_arch::basic::{PAddr, VPtr};
use sel4_common::utils::ptr_to_mut;
use sel4_common::{
    arch::vm_rights_t,
    sel4_config::{PT_INDEX_BITS, SEL4_PAGE_BITS, SEL4_PAGE_TABLE_BITS},
    utils::convert_ref_type_to_usize,
};

#[allow(unused)]
pub enum VMPageSize {
    ARMSmallPage = 0,
    ARMLargePage,
    ARMHugePage,
}

#[allow(unused)]
impl VMPageSize {
    /// Get VMPageSize from usize
    pub fn try_from_usize(value: usize) -> Option<VMPageSize> {
        match value {
            0 => Some(VMPageSize::ARMSmallPage),
            1 => Some(VMPageSize::ARMLargePage),
            2 => Some(VMPageSize::ARMHugePage),
            _ => None,
        }
    }
}

#[allow(unused)]
pub enum pte_tag_t {
    pte_table = 3,
    pte_page = 1,
    pte_4k_page = 7,
    pte_invalid = 0,
}

bitflags::bitflags! {
    /// Possible flags for a page table entry.
    pub struct PTEFlags: usize {
        // Attribute fields in stage 1 VMSAv8-64 Block and Page descriptors:
        /// Whether the descriptor is valid.
        const VALID =       bit!(0);
        /// The descriptor gives the address of the next level of translation table or 4KB page.
        /// (not a 2M, 1G block)
        const NON_BLOCK =   bit!(1);
        /// Memory attributes index field.
        const ATTR_INDX =   0b111 << 2;
        const NORMAL_NONCACHE = 0b010 << 2;
        const NORMAL =      0b100 << 2;
        /// Non-secure bit. For memory accesses from Secure state, specifies whether the output
        /// address is in Secure or Non-secure memory.
        const NS =          bit!(5);
        /// Access permission: accessable at EL0.
        const AP_EL0 =      bit!(6);
        /// Access permission: read-only.
        const AP_RO =       bit!(7);
        /// Shareability: Inner Shareable (otherwise Outer Shareable).
        const INNER =       bit!(8);
        /// Shareability: Inner or Outer Shareable (otherwise Non-shareable).
        const SHAREABLE =   bit!(9);
        /// The Access flag.
        const AF =          bit!(10);
        /// The not global bit.
        const NG =          bit!(11);
        /// Indicates that 16 adjacent translation table entries point to contiguous memory regions.
        const CONTIGUOUS =  bit!(52);
        /// The Privileged execute-never field.
        const PXN =         bit!(53);
        /// The Execute-never or Unprivileged execute-never field.
        const UXN =         bit!(54);

        // Next-level attributes in stage 1 VMSAv8-64 Table descriptors:

        /// PXN limit for subsequent levels of lookup.
        const PXN_TABLE =           bit!(59);
        /// XN limit for subsequent levels of lookup.
        const XN_TABLE =            bit!(60);
        /// Access permissions limit for subsequent levels of lookup: access at EL0 not permitted.
        const AP_NO_EL0_TABLE =     bit!(61);
        /// Access permissions limit for subsequent levels of lookup: write access not permitted.
        const AP_NO_WRITE_TABLE =   bit!(62);
        /// For memory accesses from Secure state, specifies the Security state for subsequent
        /// levels of lookup.
        const NS_TABLE =            bit!(63);

    }
}

impl PTE {
    pub fn new(addr: PAddr, flags: PTEFlags) -> Self {
        Self((addr.raw() & 0xfffffffff000) | flags.bits())
    }
    pub fn pte_next_table(addr: PAddr, _: bool) -> Self {
        Self::new(addr, PTEFlags::VALID | PTEFlags::NON_BLOCK)
    }

    pub fn get_page_base_address(&self) -> PAddr {
        paddr!(self.0 & 0xfffffffff000)
    }

    pub fn get_pte_from_ppn_mut(&self) -> &mut PTE {
        paddr!(self.get_ppn() << SEL4_PAGE_TABLE_BITS)
            .to_pptr()
            .get_mut_ref::<PTE>()
    }

    pub fn get_ppn(&self) -> usize {
        (self.0 & 0xfffffffff000) >> 10
    }

    #[inline]
    pub const fn pte_is_page_type(&self) -> bool {
        self.get_type() == (pte_tag_t::pte_4k_page) as usize
            || self.get_type() == (pte_tag_t::pte_page) as usize
    }
    pub fn is_pte_table(&self) -> bool {
        self.get_type() != pte_tag_t::pte_table as usize
    }
    pub fn get_valid(&self) -> usize {
        (self.get_type() != pte_tag_t::pte_invalid as usize) as usize
    }

    pub fn pte_table_get_present(&self) -> bool {
        self.get_type() != pte_tag_t::pte_table as usize
    }

    #[inline]
    pub fn update(&mut self, pte: Self) {
        *self = pte;
        clean_by_va_pou(
            convert_ref_type_to_usize(self),
            paddr!(convert_ref_type_to_usize(self)),
        );
    }

    /// Convert vm_rights_t to AP/S2AP bits (bits[7:6] of the PTE descriptor).
    ///
    /// Stage-1 translation AP format:
    ///   AP[2:1] = 00: EL1 RW, EL0 none
    ///   AP[2:1] = 01: EL1 RW, EL0 RW
    ///   AP[2:1] = 10: EL1 R,  EL0 none
    ///   AP[2:1] = 11: EL1 R,  EL0 R
    ///
    /// Stage-2 translation S2AP format (used when feature="hypervisor"):
    ///   S2AP[1:0] = 00: None
    ///   S2AP[1:0] = 01: Read-only
    ///   S2AP[1:0] = 10: Write-only
    ///   S2AP[1:0] = 11: Read-write
    ///
    /// Returns the raw 2-bit value to be placed in bits[7:6] of the PTE.
    pub fn ap_from_vm_rights_t(rights: vm_rights_t) -> usize {
        match rights {
            #[cfg(feature = "hypervisor")]
            vm_rights_t::VMKernelOnly => 0, // S2AP=00: None
            #[cfg(not(feature = "hypervisor"))]
            vm_rights_t::VMKernelOnly => 0, // AP[2:1]=00: EL1 RW, EL0 none

            #[cfg(feature = "hypervisor")]
            vm_rights_t::VMReadWrite => 3, // S2AP=11: Read-write
            #[cfg(not(feature = "hypervisor"))]
            vm_rights_t::VMReadWrite => 1, // AP[2:1]=01: EL1 RW, EL0 RW

            #[cfg(feature = "hypervisor")]
            vm_rights_t::VMReadOnly => 1, // S2AP=01: Read-only
            #[cfg(not(feature = "hypervisor"))]
            vm_rights_t::VMReadOnly => 3, // AP[2:1]=11: EL1 R, EL0 R
        }
    }

    /// Create a user page PTE (either 4k or large page).
    ///
    /// When `feature="hypervisor"` is enabled, the user page tables are used
    /// for Stage-2 translation (VTTBR_EL2). In this case:
    ///   - nG (not global) bit[11] is 0 (Stage-2 has no global bit)
    ///   - Memory attribute index uses S2_NORMAL (15) for cacheable pages
    ///     (MAIR_EL2), but still DEVICE_nGnRnE (0) for device pages since
    ///     stage-1 and stage-2 device attribute indices 0-3 are identical.
    ///   - AP bits use the S2AP format
    pub fn make_user_pte(
        paddr: PAddr,
        rights: vm_rights_t,
        attr: vm_attributes_t,
        page_size: usize,
    ) -> Self {
        let nonexecutable = attr.get_arm_execute_never();
        let cacheable = attr.get_arm_page_cachable();

        #[cfg(feature = "hypervisor")]
        let (nG, attrindx) = {
            if cacheable {
                (0, mair_types::S2_NORMAL as usize)
            } else {
                (0, mair_types::DEVICE_nGnRnE as usize)
            }
        };
        #[cfg(not(feature = "hypervisor"))]
        let (nG, attrindx) = {
            if cacheable {
                (1, mair_types::NORMAL as usize)
            } else {
                (1, mair_types::DEVICE_nGnRnE as usize)
            }
        };

        let vm_right = Self::ap_from_vm_rights_t(rights);
        let shareable = if cacheable && cfg!(feature = "enable_smp") { 3 } else { 0 };
        if VMPageSize::ARMSmallPage as usize == page_size {
            PTE::pte_new_4k_page(
                nonexecutable as usize,
                paddr,
                nG,
                1,
                shareable,
                vm_right,
                attrindx,
            )
        } else {
            PTE::pte_new_page(
                nonexecutable as usize,
                paddr,
                nG,
                1,
                shareable,
                vm_right,
                attrindx,
            )
        }
    }

    pub fn pte_new_table(pt_base_address: PAddr) -> PTE {
        let val = 0 | (pt_base_address.raw() & 0xfffffffff000) | (0x3);
        PTE(val)
    }

    pub fn pte_new_page(
        UXN: usize,
        page_base_address: PAddr,
        nG: usize,
        AF: usize,
        SH: usize,
        AP: usize,
        AttrIndx: usize,
    ) -> PTE {
        let val = 0
            | (UXN & 0x1) << 54
            | (page_base_address.raw() & 0xfffffffff000) >> 0
            | (nG & 0x1) << 11
            | (AF & 0x1) << 10
            | (SH & 0x3) << 8
            | (AP & 0x3) << 6
            | (AttrIndx & 0x7) << 2
            | (0x1 << 0);

        PTE(val)
    }

    pub fn pte_new_4k_page(
        UXN: usize,
        page_base_address: PAddr,
        nG: usize,
        AF: usize,
        SH: usize,
        AP: usize,
        AttrIndx: usize,
    ) -> PTE {
        let val = 0
            | (UXN & 0x1) << 54
            | (page_base_address.raw() & 0xfffffffff000) >> 0
            | (nG & 0x1) << 11
            | (AF & 0x1) << 10
            | (SH & 0x3) << 8
            | (AP & 0x3) << 6
            | (AttrIndx & 0x7) << 2
            | 0x400000000000003;
        PTE(val)
    }
    ///用于记录某个虚拟地址`vptr`对应的pte表项在内存中的位置
    pub fn lookup_pt_slot(&mut self, vptr: VPtr) -> lookupPTSlot_ret_t {
        let mut pt = self.0 as *mut PTE;
        let mut level: usize = UPT_LEVELS - 1;
        let ptBitsLeft = PT_INDEX_BITS * level + SEL4_PAGE_BITS;
        pt = unsafe { pt.add((vptr.raw() >> ptBitsLeft) & mask_bits!(VSPACE_INDEX_BITS)) };
        let mut ret: lookupPTSlot_ret_t = lookupPTSlot_ret_t {
            ptSlot: pt,
            ptBitsLeft: ptBitsLeft,
        };

        while ptr_to_mut(ret.ptSlot).get_type() == (pte_tag_t::pte_table) as usize && level > 0 {
            level = level - 1;
            ret.ptBitsLeft = ret.ptBitsLeft - PT_INDEX_BITS;
            let paddr = ptr_to_mut(ret.ptSlot).next_level_paddr();
            pt = paddr.to_pptr().get_mut_ptr::<PTE>();
            pt = unsafe { pt.add((vptr.raw() >> ret.ptBitsLeft) & mask_bits!(PT_INDEX_BITS)) };
            ret.ptSlot = pt;
        }
        ret
    }
}