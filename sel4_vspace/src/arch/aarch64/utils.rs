use crate::arch::VAddr;
use rel4_arch::basic::{PAddr, PPtr};
use sel4_common::{
    arch::{config::KERNEL_ELF_BASE_OFFSET, vm_rights_t},
    sel4_config::*,
    utils::convert_to_mut_slice,
};

pub const KPT_LEVELS: usize = 4;
/// User page table levels.
/// Non-hypervisor: 4 levels (PGD → PUD → PD → PT).
/// Hypervisor 40-bit PA (QEMU, SL0=1): 3 levels, PGD skipped, VSPACE_INDEX_BITS=10 for concatenated PUD root.
/// Hypervisor 44-bit PA (RPI4, SL0=2): 4 levels, same as non-hypervisor.
/// For now, hardcoded to 4 levels (RPI4 path). TODO: make configurable per platform.
pub const UPT_LEVELS: usize = 4;
/// Virtual space index bits for the root level.
/// 4-level: 9 bits (512 entries per table, standard).
/// 3-level: 10 bits (4 concatenated PUD tables, 2048 entries for the root).
/// For now, hardcoded to 9 (RPI4 4-level path). TODO: make configurable per platform.
pub const VSPACE_INDEX_BITS: usize = 9;
pub(self) const PAGE_ADDR_MASK: usize = mask_bits!(48) & !0xfff;
/// Map ARM PT level to user PT level index.
/// For 4-level page tables (RPI4 path), the mapping is identity.
/// For 3-level (QEMU 40-bit), level 0 is skipped (n-1).
/// For now, hardcoded to identity (RPI4 4-level path). TODO: make configurable.
#[inline]
pub fn ulvl_frm_arm_pt_lvl(n: usize) -> usize {
    n
}
#[inline]
pub fn klvl_frm_arm_pt_lvl(n: usize) -> usize {
    n
}

#[inline]
pub fn get_pt_index(addr: usize) -> usize {
    (addr >> PT_INDEX_OFFSET) & mask_bits!(PT_INDEX_BITS)
}
#[inline]
pub fn get_pd_index(addr: usize) -> usize {
    (addr >> PD_INDEX_OFFSET) & mask_bits!(PD_INDEX_BITS)
}
#[inline]
pub fn get_upud_index(addr: usize) -> usize {
    (addr >> PUD_INDEX_OFFSET) & mask_bits!(UPUD_INDEX_BITS)
}
#[inline]
pub fn get_pud_index(addr: usize) -> usize {
    (addr >> PUD_INDEX_OFFSET) & mask_bits!(PUD_INDEX_BITS)
}
#[inline]
pub fn get_pgd_index(addr: usize) -> usize {
    (addr >> PGD_INDEX_OFFSET) & mask_bits!(PGD_INDEX_BITS)
}
#[inline]
pub fn kpt_level_shift(n: usize) -> usize {
    ((PT_INDEX_BITS) * (((KPT_LEVELS) - 1) - (n))) + SEL4_PAGE_BITS
}
#[inline]
pub fn upt_level_shift(n: usize) -> usize {
    ((PT_INDEX_BITS) * (((UPT_LEVELS) - 1) - (n))) + SEL4_PAGE_BITS
}
#[inline]
pub fn get_ulvl_pgsize_bits(n: usize) -> usize {
    upt_level_shift(n)
}
#[inline]
pub fn get_ulvl_pgsize(n: usize) -> usize {
    bit!(upt_level_shift(n))
}

#[inline]
pub fn kpptr_to_paddr(x: usize) -> PAddr {
    paddr!(x - KERNEL_ELF_BASE_OFFSET)
}

impl VAddr {
    pub(super) fn get_kpt_index(&self, n: usize) -> usize {
        ((self.0) >> (kpt_level_shift(n))) & mask_bits!(PT_INDEX_BITS)
    }
    pub(super) fn get_upt_index(&self, n: usize) -> usize {
        let shift = upt_level_shift(n);
        // In hypervisor (SL0=1) mode the root level (n=0) uses 10-bit
        // VSPACE_INDEX_BITS for concatenated PUD tables.
        // Mirrors C: UPT_INDEX_MASK(n) = (n==0 ? seL4_VSpaceIndexBits : PT_INDEX_BITS)
        let mask = if n == 0 { VSPACE_INDEX_BITS } else { PT_INDEX_BITS };
        ((self.0) >> shift) & mask_bits!(mask)
    }

    /// Get the index of the pt(last level, bit 12..20)
    pub(super) const fn pt_index(&self) -> usize {
        (self.0 >> 12) & 0x1ff
    }

    /// Get the index of the pd(third level, bit 21..29)
    pub(super) const fn pd_index(&self) -> usize {
        (self.0 >> 21) & 0x1ff
    }

    /// Get the index of the pud(second level, bit 30..39)
    /// In hypervisor mode (SL0=1), the root table IS the PUD level
    /// and hardware uses VSPACE_INDEX_BITS (10) for indexing.
    pub(super) const fn pud_index(&self) -> usize {
        (self.0 >> 30) & mask_bits!(VSPACE_INDEX_BITS)
    }

    /// Get the index of the pgd(first level, bit 39..47)
    pub(super) const fn pgd_index(&self) -> usize {
        (self.0 >> 39) & 0x1ff
    }
}

/// Get the slice of the page_table items
///
/// Addr should be virtual address.
pub(super) fn page_slice<T>(addr: PPtr) -> &'static mut [T] {
    // The size of the page_table is 4K
    // The size of the item is sizeof::<usize>() bytes
    // 4096 / sizeof::<usize>() == 512
    // So the len is 512
    convert_to_mut_slice::<T>(addr.raw(), 0x200)
}

/// Get the slice for the VSpace root table.
/// In hypervisor mode (SL0=1), the root PUD needs 1024 entries (2 pages).
/// In non-hyp mode, the PGD needs 512 entries (1 page).
pub(super) fn vspace_root_slice<T>(addr: PPtr) -> &'static mut [T] {
    convert_to_mut_slice::<T>(addr.raw(), bit!(VSPACE_INDEX_BITS))
}

pub fn ap_from_vm_rights(rights: vm_rights_t) -> usize {
    // match rights {
    //     vm_rights_t::VMKernelOnly => 0,
    //     vm_rights_t::VMReadWrite => 1,
    //     vm_rights_t::VMReadOnly => 3,
    // }
    rights as usize
}

// #[repr(C)]
// #[derive(Debug, Clone, Copy)]
// pub struct PGDE(pub usize);
// #[repr(C)]
// #[derive(Debug, Clone, Copy)]
// pub struct PUDE(pub usize);
// #[repr(C)]
// #[derive(Debug, Clone, Copy)]
// pub struct PDE(pub usize);
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PTE(pub usize);

#[repr(C)]
#[derive(Debug, Clone)]
pub struct ASID(usize);

// // Implemente generic function for PGDE PUDE PDE
// impl_multi!(PGDE, PUDE, PDE {
//     /// Get the slice of the next level page.
//     ///
//     /// PGDE -> PUDE[PAGE_ITEMS]
//     #[inline]
//     pub fn next_level_slice<T>(&self) -> &'static mut [T] {
//         page_slice(paddr_to_pptr(self.next_level_paddr()))
//     }
// });

impl PTE {
    #[inline]
    pub fn get_ptr(&self) -> usize {
        self as *const Self as usize
    }

    #[inline]
    pub fn get_mut_ptr(&mut self) -> usize {
        self as *mut Self as usize
    }

    /// Get the next level paddr
    #[inline]
    pub const fn next_level_paddr(&self) -> PAddr {
        paddr!(self.0 & PAGE_ADDR_MASK)
    }
    /// Set the next level paddr
    ///
    /// If It is PT or HUGE_PAGE, it will set the maped physical address
    /// Else it is the page to contains list
    #[inline]
    pub fn set_next_level_paddr(&mut self, addr: PAddr) {
        self.0 = (self.0 & !PAGE_ADDR_MASK) | (addr.raw() & PAGE_ADDR_MASK);
    }
    /// Set Page Attribute
    #[inline]
    pub fn set_attr(&mut self, value: usize) {
        self.0 = (self.0 & PAGE_ADDR_MASK) | (value & !PAGE_ADDR_MASK);
    }
    /// Get the address of the self.
    #[inline]
    pub fn self_addr(&self) -> usize {
        self as *const _ as _
    }
    /// Get the attribute of self
    #[inline]
    pub const fn attr(&self) -> usize {
        self.0 & !PAGE_ADDR_MASK
    }
    /// Create self through addr and attributes.
    #[inline]
    pub const fn new_page(addr: PAddr, sign: usize) -> Self {
        Self((addr.raw() & PAGE_ADDR_MASK) | (sign & !PAGE_ADDR_MASK))
    }

    /// Get the page's type info
    #[inline]
    pub const fn get_type(&self) -> usize {
        self.0 & 0x3 | ((self.0 & (1 << 58)) >> 56)
    }

    #[inline]
    pub const fn new_from_pte(word: usize) -> Self {
        Self(word)
    }

    #[inline]
    pub fn invalidate(&mut self) {
        self.0 = 0;
    }

    #[inline]
    pub fn next_level_slice<T>(&self) -> &'static mut [T] {
        page_slice(self.next_level_paddr().to_pptr())
    }
}

impl PTE {
    #[inline]
    pub const fn get_reserved(&self) -> usize {
        self.0 & 0x3
    }

    #[inline]
    pub const fn is_present(&self) -> bool {
        self.get_reserved() == 0x3
    }

    #[inline]
    pub const fn pte_ptr_get_page_base_address(&self) -> PAddr {
        paddr!(self.0 & 0xfffffffff000)
    }
}
