use core::intrinsics::unlikely;
use core::ops::{Deref, DerefMut};

use super::pte::pte_tag_t;
use super::{kpptr_to_paddr, machine::*, UPT_LEVELS};
use crate::arch::VAddr;
use crate::{asid_t, find_vspace_for_asid, paddr_to_pptr, pptr_t, pptr_to_paddr, vptr_t, PTE};
use sel4_common::arch::MessageLabel;
use sel4_common::structures::exception_t;
use sel4_common::structures_gen::{cap, cap_tag, cap_vspace_cap};
use sel4_common::utils::{pageBitsForSize, ptr_to_mut};
use sel4_common::{
    sel4_config::{seL4_PageBits, PT_INDEX_BITS},
    structures_gen::lookup_fault,
    BIT,
};
use sel4_cspace::capability::cap_arch_func;
pub const PAGE_ALIGNED_LEN: usize = BIT!(PT_INDEX_BITS);
#[repr(align(4096))]
#[derive(Clone, Copy)]
pub struct PageAligned<T>([T; PAGE_ALIGNED_LEN]);

impl<T: Copy> PageAligned<T> {
    pub const fn new(v: T) -> Self {
        Self([v; PAGE_ALIGNED_LEN])
    }
}

impl<T> Deref for PageAligned<T> {
    type Target = [T; PAGE_ALIGNED_LEN];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for PageAligned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPGD: PageAligned<PTE> = PageAligned::new(PTE(0));

#[inline]
pub fn get_kernel_page_global_directory_base() -> usize {
    unsafe { armKSGlobalKernelPGD.as_ptr() as usize }
}

#[inline]
pub fn set_kernel_page_global_directory_by_index(idx: usize, pgde: PTE) {
    unsafe {
        armKSGlobalKernelPGD[idx] = pgde;
    }
}

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPUD: PageAligned<PTE> = PageAligned::new(PTE(0));

#[inline]
pub fn get_kernel_page_upper_directory_base() -> usize {
    unsafe { armKSGlobalKernelPUD.as_ptr() as usize }
}

#[inline]
pub fn set_kernel_page_upper_directory_by_index(idx: usize, pude: PTE) {
    unsafe {
        armKSGlobalKernelPUD[idx] = pude;
    }
}
// #[no_mangle]
// #[link_section = ".page_table"]
// pub(crate) static mut armKSGlobalKernelPDs: [[PTE; BIT!(PT_INDEX_BITS)]; BIT!(PT_INDEX_BITS)] =
//     [[PTE(0); BIT!(PT_INDEX_BITS)]; BIT!(PT_INDEX_BITS)];
#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPDs: PageAligned<PageAligned<PTE>> =
    PageAligned::new(PageAligned::new(PTE(0)));

#[inline]
pub fn get_kernel_page_directory_base_by_index(idx: usize) -> usize {
    unsafe { armKSGlobalKernelPDs[idx].as_ptr() as usize }
}

#[inline]
pub fn set_kernel_page_directory_by_index(idx1: usize, idx2: usize, pde: PTE) {
    unsafe {
        armKSGlobalKernelPDs[idx1][idx2] = pde;
    }
}

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalUserVSpace: PageAligned<PTE> = PageAligned::new(PTE(0));

#[inline]
pub fn get_arm_global_user_vspace_base() -> usize {
    unsafe { armKSGlobalUserVSpace.as_ptr() as usize }
}

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPT: PageAligned<PTE> = PageAligned::new(PTE(0));

#[inline]
pub fn get_kernel_page_table_base() -> usize {
    unsafe { armKSGlobalKernelPT.as_ptr() as usize }
}

#[inline]
pub fn set_kernel_page_table_by_index(idx: usize, pte: PTE) {
    unsafe {
        armKSGlobalKernelPT[idx] = pte;
    }
}
/// 根据给定的`vspace_root`设置相应的页表，会检查`vspace_root`是否合法，如果不合法默认设置为内核页表
///
/// Use page table in vspace_root to set the satp register.
pub fn set_vm_root(thread_root: &cap) -> Result<(), lookup_fault> {
    if !thread_root.is_valid_native_root() {
        set_current_user_vspace_root(ttbr_new(
            0,
            kpptr_to_paddr(get_arm_global_user_vspace_base()),
        ));
        return Ok(());
    }
    let thread_root_vspace = cap::cap_vspace_cap(&thread_root);
    let vspace_root = thread_root_vspace.get_capVSBasePtr() as usize;
    let asid = thread_root_vspace.get_capVSMappedASID() as usize;
    let find_ret = find_vspace_for_asid(asid);

    if let Some(root) = find_ret.vspace_root {
        if find_ret.status != exception_t::EXCEPTION_NONE || root as usize != vspace_root {
            set_current_user_vspace_root(ttbr_new(
                0,
                kpptr_to_paddr(get_arm_global_user_vspace_base()),
            ));
            return Ok(());
        }
    }
    set_current_user_vspace_root(pptr_to_paddr(thread_root_vspace.get_capVSBasePtr() as usize));
    Ok(())
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn activate_kernel_vspace() {
    clean_invalidate_l1_caches();
    set_current_kernel_vspace_root(ttbr_new(
        0,
        kpptr_to_paddr(get_kernel_page_global_directory_base()),
    ));
    set_current_user_vspace_root(ttbr_new(
        0,
        kpptr_to_paddr(get_arm_global_user_vspace_base()),
    ));
    invalidate_local_tlb();
    /* A53 hardware does not support TLB locking */
}

// pub fn make_user_1st_level(
//     paddr: pptr_t,
//     vm_rights: vm_rights_t,
//     attributes: vm_attributes_t,
// ) -> PUDE {
//     PUDE::new_1g(
//         attributes.get_arm_execute_never(),
//         paddr,
//         1,
//         1,
//         0,
//         ap_from_vm_rights(vm_rights),
//         attributes.get_attr_index(),
//     )
// }

// pub fn make_user_2nd_level(
//     paddr: pptr_t,
//     vm_rights: vm_rights_t,
//     attributes: vm_attributes_t,
// ) -> PDE {
//     PDE::new_large(
//         attributes.get_arm_execute_never(),
//         paddr,
//         1,
//         1,
//         0,
//         ap_from_vm_rights(vm_rights),
//         attributes.get_attr_index(),
//     )
// }

// pub fn makeUser3rdLevel(paddr: pptr_t, vm_rights: vm_rights_t, attributes: vm_attributes_t) -> PTE {
//     PTE::pte_new(
//         attributes.get_arm_execute_never() as usize,
//         paddr,
//         1,
//         1,
//         0,
//         ap_from_vm_rights(vm_rights),
//         attributes.get_attr_index() as usize,
//         3, // RESERVED
//     )
// }

#[no_mangle]
pub fn set_vm_root_for_flush_with_thread_root(
    vspace: *mut PTE,
    asid: asid_t,
    thread_root: &cap_vspace_cap,
) -> bool {
    if thread_root.clone().unsplay().get_tag() == cap_tag::cap_vspace_cap
        && thread_root.get_capVSIsMapped() != 0
        && thread_root.get_capVSBasePtr() == vspace as u64
    {
        return false;
    }

    // armv_context_switch(vspace, asid);
    set_current_user_vspace_root(ttbr_new(asid, vspace as usize));
    true
}

// pub fn page_upper_directory_mapped(asid: asid_t, vaddr: vptr_t, pud: &PUDE) -> Option<*mut PGDE> {
//     match find_map_for_asid(asid) {
//         Some(asid_map) => {
//             let lookup_ret = PGDE::new_from_pte(asid_map.get_vspace_root()).lookup_pgd_slot(vaddr);
//             if lookup_ret.status != exception_t::EXCEPTION_NONE {
//                 return None;
//             }

//             let slot = unsafe { &mut (*lookup_ret.pgdSlot) };

//             if !slot.get_present()
//                 || slot.get_pud_base_address() != pptr_to_paddr(pud as *const _ as _)
//             {
//                 return None;
//             }

//             return Some(slot);
//         }
//         None => None,
//     }
// }

// pub fn page_directory_mapped(asid: asid_t, vaddr: vptr_t, pd: &PDE) -> Option<*mut PUDE> {
//     match find_map_for_asid(asid) {
//         Some(asid_map) => {
//             let lookup_ret = PGDE::new_from_pte(asid_map.get_vspace_root()).lookup_pud_slot(vaddr);
//             if lookup_ret.status != exception_t::EXCEPTION_NONE {
//                 return None;
//             }

//             let slot = unsafe { &mut (*lookup_ret.pudSlot) };

//             if !slot.get_present()
//                 || slot.get_pd_base_address() != pptr_to_paddr(pd as *const _ as _)
//             {
//                 return None;
//             }

//             return Some(slot);
//         }
//         None => None,
//     }
// }

/// TODO: Make pt as usize of
// pub fn page_table_mapped(asid: asid_t, vaddr: vptr_t, pt: &PTE) -> Option<*mut PDE> {
//     match find_map_for_asid(asid) {
//         Some(asid_map) => {
//             let lookup_ret = PGDE::new_from_pte(asid_map.get_vspace_root()).lookup_pd_slot(vaddr);
//             if lookup_ret.status != exception_t::EXCEPTION_NONE {
//                 return None;
//             }

//             let slot = unsafe { &mut (*lookup_ret.pdSlot) };

//             if !slot.get_present() || slot.get_base_address() != pptr_to_paddr(pt.0) {
//                 return None;
//             }

//             return Some(slot);
//         }
//         None => None,
//     }
// }

#[inline]
pub fn invalidate_tlb_by_asid(asid: asid_t) {
    invalidate_local_tlb_asid(asid);
}

#[inline]
pub fn invalidate_tlb_by_asid_va(asid: asid_t, vaddr: vptr_t) {
    invalidate_local_tlb_va_asid((asid << 48) | vaddr >> seL4_PageBits);
}

// pub fn unmap_page_upper_directory(asid: asid_t, vaddr: vptr_t, pud: &PUDE) {
//     match page_upper_directory_mapped(asid, vaddr, pud) {
//         Some(slot) => {
//             let slot = unsafe { &mut (*slot) };
//             slot.invalidate();
//             clean_by_va_pou(slot.get_ptr(), pptr_to_paddr(slot.get_ptr()));
//             invalidate_tlb_by_asid(asid);
//         }
//         None => {}
//     }
// }

// pub fn unmap_page_directory(asid: asid_t, vaddr: vptr_t, pd: &PDE) {
//     match page_directory_mapped(asid, vaddr, pd) {
//         Some(slot) => {
//             let slot = unsafe { &mut (*slot) };
//             slot.invalidate();
//             clean_by_va_pou(slot.get_ptr(), pptr_to_paddr(slot.get_ptr()));
//             invalidate_tlb_by_asid(asid);
//         }
//         None => {}
//     }
// }

pub fn unmap_page_table(asid: asid_t, vaddr: vptr_t, pt: &PTE) {
    let find_ret = find_vspace_for_asid(asid);
    if find_ret.status != exception_t::EXCEPTION_NONE {
        return;
    }
    let mut ptSlot: *mut PTE = core::ptr::null_mut::<PTE>();
    let mut pte = find_ret.vspace_root.unwrap();
    let mut level: usize = 0;
    while level < UPT_LEVELS - 1 && pte as usize != pt as *const PTE as usize {
        ptSlot = unsafe { pte.add(VAddr(vaddr).get_upt_index(level)) };
        if ptr_to_mut(ptSlot).get_type() != (pte_tag_t::pte_table) as usize {
            return;
        }
        pte = paddr_to_pptr(ptr_to_mut(ptSlot).next_level_paddr()) as *mut PTE;
        level = level + 1;
    }
    if pte as usize != pt as *const PTE as usize {
        return;
    }
    assert!(!ptSlot.is_null());
    unsafe {
        *(ptSlot) = PTE(0);
        ptr_to_mut(ptSlot).update(*(pte));
    }
    invalidate_tlb_by_asid(asid);
}

/// Unmap a page table
/// TODO: Remove result Result<(), lookup_fault_t>
pub fn unmap_page(
    page_size: usize,
    asid: asid_t,
    vptr: vptr_t,
    pptr: pptr_t,
) -> Result<(), lookup_fault> {
    let addr = pptr_to_paddr(pptr);
    let find_ret = find_vspace_for_asid(asid);
    if unlikely(find_ret.status != exception_t::EXCEPTION_NONE) {
        return Ok(());
    }
    let lu_ret = PTE::new_from_pte(find_ret.vspace_root.unwrap() as usize).lookup_pt_slot(vptr);
    if unlikely(lu_ret.ptBitsLeft != pageBitsForSize(page_size)) {
        return Ok(());
    }

    let pte = ptr_to_mut(lu_ret.ptSlot);
    if !(pte.get_type() == (pte_tag_t::pte_4k_page) as usize
        || pte.get_type() == (pte_tag_t::pte_page) as usize)
    {
        return Ok(());
    }
    if pte.get_page_base_address() != addr {
        return Ok(());
    }
    unsafe {
        *(lu_ret.ptSlot) = PTE(0);
        pte.update(*(lu_ret.ptSlot));
    }
    assert!(asid < BIT!(16));
    invalidate_tlb_by_asid(asid);
    Ok(())

    // match page_size {
    //     ARM_Small_Page => {
    //         let lu_ret =
    //             PGDE::new_from_pte(find_ret.vspace_root.unwrap() as usize).lookup_pt_slot(vptr);
    //         if unlikely(lu_ret.status != exception_t::EXCEPTION_NONE) {
    //             return Ok(());
    //         }
    //         let pte = ptr_to_mut(lu_ret.ptSlot);
    //         if pte.is_present() && pte.pte_ptr_get_page_base_address() == addr {
    //             *pte = PTE(0);
    //             unsafe { core::arch::asm!("tlbi vmalle1; dsb sy; isb") };
    //             clean_by_va_pou(
    //                 convert_ref_type_to_usize(pte),
    //                 paddr_to_pptr(convert_ref_type_to_usize(pte)),
    //             );
    //         }
    //         Ok(())
    //     }
    //     ARM_Large_Page => {
    //         log::info!("unmap large page: {:#x?}", vptr);
    //         let lu_ret =
    //             PGDE::new_from_pte(find_ret.vspace_root.unwrap() as usize).lookup_pd_slot(vptr);
    //         if unlikely(lu_ret.status != exception_t::EXCEPTION_NONE) {
    //             return Ok(());
    //         }
    //         let pde = ptr_to_mut(lu_ret.pdSlot);
    //         if pde.get_present() && pde.get_base_address() == addr {
    //             *pde = PDE(0);
    //             unsafe { core::arch::asm!("tlbi vmalle1; dsb sy; isb") };
    //             clean_by_va_pou(
    //                 convert_ref_type_to_usize(pde),
    //                 paddr_to_pptr(convert_ref_type_to_usize(pde)),
    //             );
    //         }
    //         Ok(())
    //     }
    //     _ => unimplemented!("unMapPage: {page_size}"),
    // }
    /*
        switch (page_size) {
        case ARMLargePage: {
            lookupPDSlot_ret_t lu_ret;
            lu_ret = lookupPDSlot(find_ret.vspace_root, vptr);
            if (unlikely(lu_ret.status != EXCEPTION_NONE)) {
                return;
            }
            if (pde_pde_large_ptr_get_present(lu_ret.pdSlot) &&
                pde_pde_large_ptr_get_page_base_address(lu_ret.pdSlot) == addr) {
                *(lu_ret.pdSlot) = pde_invalid_new();
                cleanByVA_PoU((vptr_t)lu_ret.pdSlot, pptr_to_paddr(lu_ret.pdSlot));
            }
            break;
        }
        case ARMHugePage: {
            lookupPUDSlot_ret_t lu_ret;
            lu_ret = lookupPUDSlot(find_ret.vspace_root, vptr);
            if (unlikely(lu_ret.status != EXCEPTION_NONE)) {
                return;
            }
            if (pude_pude_1g_ptr_get_present(lu_ret.pudSlot) &&
                pude_pude_1g_ptr_get_page_base_address(lu_ret.pudSlot) == addr) {
                *(lu_ret.pudSlot) = pude_invalid_new();
                cleanByVA_PoU((vptr_t)lu_ret.pudSlot, pptr_to_paddr(lu_ret.pudSlot));
            }
            break;
        }
        default:
            fail("Invalid ARM page type");
        }
        assert(asid < BIT(16));
        invalidateTLBByASIDVA(asid, vptr);
    */
}

pub fn do_flush(invLabel: MessageLabel, start: usize, end: usize, pstart: usize) {
    match invLabel {
        MessageLabel::ARMPageClean_Data | MessageLabel::ARMVSpaceClean_Data => {
            clean_cache_range_ram(start, end, pstart)
        }
        MessageLabel::ARMPageInvalidate_Data | MessageLabel::ARMVSpaceInvalidate_Data => {
            invalidate_cache_range_ram(start, end, pstart)
        }
        MessageLabel::ARMVSpaceCleanInvalidate_Data | MessageLabel::ARMPageCleanInvalidate_Data => {
            clean_invalidate_cache_range_ram(start, end, pstart);
        }
        MessageLabel::ARMPageUnify_Instruction | MessageLabel::ARMVSpaceUnify_Instruction => {
            clean_cache_range_pou(start, end, pstart);
            dsb();
            invalidate_cache_range_i(start, end, pstart);
            isb();
        }
        _ => unimplemented!("unimplemented do_flush :{:?}", invLabel),
    };
}
