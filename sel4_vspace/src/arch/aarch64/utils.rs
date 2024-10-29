use crate::arch::VAddr;
use sel4_common::BIT;
use sel4_common::{
    arch::{
        config::{KERNEL_ELF_BASE_OFFSET, PPTR_BASE_OFFSET},
        vm_rights_t,
    },
    sel4_config::*,
    utils::convert_to_mut_slice,
    MASK,
};

pub const KPT_LEVELS: usize = 4;
pub const UPT_LEVELS: usize = 4;
pub const seL4_VSpaceIndexBits: usize = 9;
pub(self) const PAGE_ADDR_MASK: usize = MASK!(48) & !0xfff;
#[inline]
pub fn ULVL_FRM_ARM_PT_LVL(n: usize) -> usize {
    n
}
#[inline]
pub fn KLVL_FRM_ARM_PT_LVL(n: usize) -> usize {
    n
}

#[inline]
pub fn GET_PT_INDEX(addr: usize) -> usize {
    (addr >> PT_INDEX_OFFSET) & MASK!(PT_INDEX_BITS)
}
#[inline]
pub fn GET_PD_INDEX(addr: usize) -> usize {
    (addr >> PD_INDEX_OFFSET) & MASK!(PD_INDEX_BITS)
}
#[inline]
pub fn GET_UPUD_INDEX(addr: usize) -> usize {
    (addr >> PUD_INDEX_OFFSET) & MASK!(UPUD_INDEX_BITS)
}
#[inline]
pub fn GET_PUD_INDEX(addr: usize) -> usize {
    (addr >> PUD_INDEX_OFFSET) & MASK!(PUD_INDEX_BITS)
}
#[inline]
pub fn GET_PGD_INDEX(addr: usize) -> usize {
    (addr >> PGD_INDEX_OFFSET) & MASK!(PGD_INDEX_BITS)
}
#[inline]
pub fn KPT_LEVEL_SHIFT(n: usize) -> usize {
    ((PT_INDEX_BITS) * (((KPT_LEVELS) - 1) - (n))) + seL4_PageBits
}
#[inline]
pub fn UPT_LEVEL_SHIFT(n: usize) -> usize {
    ((PT_INDEX_BITS) * (((UPT_LEVELS) - 1) - (n))) + seL4_PageBits
}
#[inline]
pub fn GET_ULVL_PGSIZE_BITS(n: usize) -> usize {
    UPT_LEVEL_SHIFT(n)
}
#[inline]
pub fn GET_ULVL_PGSIZE(n: usize) -> usize {
    BIT!(UPT_LEVEL_SHIFT(n))
}

#[inline]
pub fn kpptr_to_paddr(x: usize) -> usize {
    x - KERNEL_ELF_BASE_OFFSET
}

///计算以`PPTR_BASE`作为偏移的指针虚拟地址对应的物理地址
#[inline]
pub const fn pptr_to_paddr(x: usize) -> usize {
    x - PPTR_BASE_OFFSET
}

///计算物理地址对应的虚拟地址，以`PPTR_BASE`作为偏移
#[inline]
pub fn paddr_to_pptr(x: usize) -> usize {
    x + PPTR_BASE_OFFSET
}

impl VAddr {
    pub(super) fn GET_KPT_INDEX(&self, n: usize) -> usize {
        ((self.0) >> (KPT_LEVEL_SHIFT(n))) & MASK!(PT_INDEX_BITS)
    }
    pub(super) fn GET_UPT_INDEX(&self, n: usize) -> usize {
        ((self.0) >> (UPT_LEVEL_SHIFT(n))) & MASK!(PT_INDEX_BITS)
    }

    /// Get the index of the pt(last level, bit 12..20)
    pub(super) const fn pt_index(&self) -> usize {
        (self.0 >> 12) & 0x1ff
    }

    /// Get the index of the pd(third level, bit 21..29)
    pub(super) const fn pd_index(&self) -> usize {
        (self.0 >> 21) & 0x1ff
    }

    /// Get the index of the pud(second level, bit 30..38)
    pub(super) const fn pud_index(&self) -> usize {
        (self.0 >> 30) & 0x1ff
    }

    /// Get the index of the pgd(first level, bit 39..47)
    pub(super) const fn pgd_index(&self) -> usize {
        (self.0 >> 39) & 0x1ff
    }
}

/// Get the slice of the page_table items
///
/// Addr should be virtual address.
pub(super) fn page_slice<T>(addr: usize) -> &'static mut [T] {
    assert!(addr >= KERNEL_ELF_BASE_OFFSET);
    // The size of the page_table is 4K
    // The size of the item is sizeof::<usize>() bytes
    // 4096 / sizeof::<usize>() == 512
    // So the len is 512
    convert_to_mut_slice::<T>(addr, 0x200)
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

/// Implemente generic function for given Ident
#[macro_export]
macro_rules! impl_multi {
    ($($t:ident),* {$($block:item)*}) => {
        macro_rules! methods {
            () => {
                $($block)*
            };
        }
        $(
            impl $t {
                methods!();
            }
        )*
    }
}

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

impl_multi!( PTE {
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
    pub const fn next_level_paddr(&self) -> usize {
        self.0 & PAGE_ADDR_MASK
    }
    /// Set the next level paddr
    ///
    /// If It is PT or HUGE_PAGE, it will set the maped physical address
    /// Else it is the page to contains list
    #[inline]
    pub fn set_next_level_paddr(&mut self, value: usize) {
        self.0 = (self.0 & !PAGE_ADDR_MASK) | (value & PAGE_ADDR_MASK);
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
    pub const fn new_page(addr: usize, sign: usize) -> Self {
        Self((addr & PAGE_ADDR_MASK) | (sign & !PAGE_ADDR_MASK))
    }

    /// Get the page's type info
    #[inline]
    pub const fn get_type(&self) -> usize {
        self.0 & 0x3 | ((self.0 &(1<<58) )>>56)
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
        page_slice(paddr_to_pptr(self.next_level_paddr()))
    }
});

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
    pub const fn pte_ptr_get_page_base_address(&self) -> usize {
        self.0 & 0xfffffffff000
    }
}
