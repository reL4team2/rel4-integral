use crate::{arch::aarch64::machine::clean_by_va_pou, vm_attributes_t, PTE};

use super::utils::paddr_to_pptr;
use super::{mair_types, seL4_VSpaceIndexBits, UPT_LEVELS};
use crate::{lookupPTSlot_ret_t, vptr_t};
use sel4_common::utils::ptr_to_mut;
use sel4_common::MASK;
use sel4_common::{
    arch::vm_rights_t,
    sel4_config::{seL4_PageBits, seL4_PageTableBits, PT_INDEX_BITS},
    utils::{convert_ref_type_to_usize, convert_to_mut_type_ref},
    BIT,
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
        const VALID =       BIT!(0);
        /// The descriptor gives the address of the next level of translation table or 4KB page.
        /// (not a 2M, 1G block)
        const NON_BLOCK =   BIT!(1);
        /// Memory attributes index field.
        const ATTR_INDX =   0b111 << 2;
        const NORMAL_NONCACHE = 0b010 << 2;
        const NORMAL =      0b100 << 2;
        /// Non-secure bit. For memory accesses from Secure state, specifies whether the output
        /// address is in Secure or Non-secure memory.
        const NS =          BIT!(5);
        /// Access permission: accessable at EL0.
        const AP_EL0 =      BIT!(6);
        /// Access permission: read-only.
        const AP_RO =       BIT!(7);
        /// Shareability: Inner Shareable (otherwise Outer Shareable).
        const INNER =       BIT!(8);
        /// Shareability: Inner or Outer Shareable (otherwise Non-shareable).
        const SHAREABLE =   BIT!(9);
        /// The Access flag.
        const AF =          BIT!(10);
        /// The not global bit.
        const NG =          BIT!(11);
        /// Indicates that 16 adjacent translation table entries point to contiguous memory regions.
        const CONTIGUOUS =  BIT!(52);
        /// The Privileged execute-never field.
        const PXN =         BIT!(53);
        /// The Execute-never or Unprivileged execute-never field.
        const UXN =         BIT!(54);

        // Next-level attributes in stage 1 VMSAv8-64 Table descriptors:

        /// PXN limit for subsequent levels of lookup.
        const PXN_TABLE =           BIT!(59);
        /// XN limit for subsequent levels of lookup.
        const XN_TABLE =            BIT!(60);
        /// Access permissions limit for subsequent levels of lookup: access at EL0 not permitted.
        const AP_NO_EL0_TABLE =     BIT!(61);
        /// Access permissions limit for subsequent levels of lookup: write access not permitted.
        const AP_NO_WRITE_TABLE =   BIT!(62);
        /// For memory accesses from Secure state, specifies the Security state for subsequent
        /// levels of lookup.
        const NS_TABLE =            BIT!(63);

    }
}

impl PTE {
    pub fn new(addr: usize, flags: PTEFlags) -> Self {
        Self((addr & 0xfffffffff000) | flags.bits())
    }
    pub fn pte_next_table(addr: usize, _: bool) -> Self {
        Self::new(addr, PTEFlags::VALID | PTEFlags::NON_BLOCK)
    }
    // fn new_4k_page(addr: usize, flags: PTEFlags) -> Self {
    //     Self((addr & 0xfffffffff000) | flags.bits() | 0x400000000000003)
    // }

    pub fn get_page_base_address(&self) -> usize {
        self.0 & 0xfffffffff000
    }

    pub fn get_pte_from_ppn_mut(&self) -> &mut PTE {
        convert_to_mut_type_ref::<PTE>(paddr_to_pptr(self.get_ppn() << seL4_PageTableBits))
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
            convert_ref_type_to_usize(self),
        );
    }

    pub fn ap_from_vm_rights_t(rights: vm_rights_t) -> PTEFlags {
        match rights {
            vm_rights_t::VMKernelOnly => PTEFlags::empty(),
            vm_rights_t::VMReadWrite => PTEFlags::AP_EL0,
            vm_rights_t::VMReadOnly => PTEFlags::AP_EL0 | PTEFlags::AP_RO,
        }
    }

    pub fn make_user_pte(
        paddr: usize,
        rights: vm_rights_t,
        attr: vm_attributes_t,
        page_size: usize,
    ) -> Self {
        let nonexecutable = attr.get_armExecuteNever();
        let cacheable = attr.get_armPageCacheable();
        let mut attrindx = mair_types::DEVICE_nGnRnE as usize;
        if cacheable {
            attrindx = mair_types::NORMAL as usize;
        }
        let nG: usize = 1;
        let vm_right: usize = Self::ap_from_vm_rights_t(rights).bits() >> 6;
        if VMPageSize::ARMSmallPage as usize == page_size {
            PTE::pte_new_4k_page(nonexecutable as usize, paddr, nG, 1, 0, vm_right, attrindx)
        } else {
            PTE::pte_new_page(nonexecutable as usize, paddr, nG, 1, 0, vm_right, attrindx)
        }
    }

    pub fn pte_new_table(pt_base_address: usize) -> PTE {
        let val = 0 | (pt_base_address & 0xfffffffff000) | (0x3);
        PTE(val)
    }

    pub fn pte_new_page(
        UXN: usize,
        page_base_address: usize,
        nG: usize,
        AF: usize,
        SH: usize,
        AP: usize,
        AttrIndx: usize,
    ) -> PTE {
        let val = 0
            | (UXN & 0x1) << 54
            | (page_base_address & 0xfffffffff000) >> 0
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
        page_base_address: usize,
        nG: usize,
        AF: usize,
        SH: usize,
        AP: usize,
        AttrIndx: usize,
    ) -> PTE {
        let val = 0
            | (UXN & 0x1) << 54
            | (page_base_address & 0xfffffffff000) >> 0
            | (nG & 0x1) << 11
            | (AF & 0x1) << 10
            | (SH & 0x3) << 8
            | (AP & 0x3) << 6
            | (AttrIndx & 0x7) << 2
            | 0x400000000000003;
        PTE(val)
    }
    ///用于记录某个虚拟地址`vptr`对应的pte表项在内存中的位置
    pub fn lookup_pt_slot(&mut self, vptr: vptr_t) -> lookupPTSlot_ret_t {
        let mut pt = self.0 as *mut PTE;
        let mut level: usize = UPT_LEVELS - 1;
        let ptBitsLeft = PT_INDEX_BITS * level + seL4_PageBits;
        pt = unsafe { pt.add((vptr >> ptBitsLeft) & MASK!(seL4_VSpaceIndexBits)) };
        let mut ret: lookupPTSlot_ret_t = lookupPTSlot_ret_t {
            ptSlot: pt,
            ptBitsLeft: ptBitsLeft,
        };

        while ptr_to_mut(ret.ptSlot).get_type() == (pte_tag_t::pte_table) as usize && level > 0 {
            level = level - 1;
            ret.ptBitsLeft = ret.ptBitsLeft - PT_INDEX_BITS;
            let paddr = ptr_to_mut(ret.ptSlot).next_level_paddr();
            pt = paddr_to_pptr(paddr) as *mut PTE;
            pt = unsafe { pt.add((vptr >> ret.ptBitsLeft) & MASK!(PT_INDEX_BITS)) };
            ret.ptSlot = pt;
        }
        ret
    }
}
