use super::pte::pte_tag_t;
use super::{kpptr_to_paddr, machine::*, UPT_LEVELS};
use crate::arch::VAddr;
use crate::utils::PageAligned;
use crate::{asid_t, find_vspace_for_asid, PTE};
use core::intrinsics::unlikely;
use rel4_arch::basic::{PAddr, PPtr, VPtr};
use sel4_common::arch::MessageLabel;
use sel4_common::structures::exception_t;
use sel4_common::structures_gen::{cap, cap_tag, cap_vspace_cap};
use sel4_common::utils::{pageBitsForSize, ptr_to_mut};
use sel4_common::{sel4_config::SEL4_PAGE_BITS, structures_gen::lookup_fault};
use sel4_cspace::capability::cap_arch_func;

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPGD: PageAligned<PTE> = PageAligned::new(PTE(0));

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPUD: PageAligned<PTE> = PageAligned::new(PTE(0));

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPDs: PageAligned<PageAligned<PTE>> =
    PageAligned::new(PageAligned::new(PTE(0)));

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPT: PageAligned<PTE> = PageAligned::new(PTE(0));

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalUserVSpace: PageAligned<PTE> = PageAligned::new(PTE(0));

#[inline]
pub fn get_kernel_page_global_directory_base() -> usize {
    &raw const armKSGlobalKernelPGD as usize
}

#[inline]
pub fn set_kernel_page_global_directory_by_index(idx: usize, pgde: PTE) {
    unsafe { armKSGlobalKernelPGD[idx] = pgde }
}

#[inline]
pub fn get_kernel_page_upper_directory_base() -> usize {
    &raw const armKSGlobalKernelPUD as usize
}

#[inline]
pub fn set_kernel_page_upper_directory_by_index(idx: usize, pude: PTE) {
    unsafe { armKSGlobalKernelPUD[idx] = pude }
}

#[inline]
pub fn get_kernel_page_directory_base_by_index(idx: usize) -> usize {
    unsafe { armKSGlobalKernelPDs[idx].as_ptr() as usize }
}

#[inline]
pub fn set_kernel_page_directory_by_index(idx1: usize, idx2: usize, pde: PTE) {
    unsafe { armKSGlobalKernelPDs[idx1][idx2] = pde }
}

#[inline]
pub fn get_arm_global_user_vspace_base() -> usize {
    &raw const armKSGlobalUserVSpace as usize
}

#[inline]
pub fn get_kernel_page_table_base() -> usize {
    &raw const armKSGlobalKernelPT as usize
}

#[inline]
pub fn set_kernel_page_table_by_index(idx: usize, pte: PTE) {
    unsafe { armKSGlobalKernelPT[idx] = pte }
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
    set_current_user_vspace_root(
        pptr!(thread_root_vspace.get_capVSBasePtr())
            .to_paddr()
            .raw(),
    );
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
    set_current_user_vspace_root(ttbr_new(asid, paddr!(vspace)));
    true
}

#[inline]
pub fn invalidate_tlb_by_asid(asid: asid_t) {
    invalidate_local_tlb_asid(asid);
    #[cfg(feature = "enable_smp")]
    {
        extern "C" {
            fn remote_invalidate_tlb_asid(asid: asid_t);
        }
        unsafe {
            remote_invalidate_tlb_asid(asid);
        }
    }
}

#[inline]
pub fn invalidate_tlb_by_asid_va(asid: asid_t, vaddr: VPtr) {
    invalidate_local_tlb_va_asid((asid << 48) | vaddr.raw() >> SEL4_PAGE_BITS);
    #[cfg(feature = "enable_smp")]
    {
        extern "C" {
            fn remote_invalidate_translation_single(vptr: usize);
        }
        unsafe {
            remote_invalidate_translation_single((asid << 48) | vaddr.raw() >> SEL4_PAGE_BITS);
        }
    }
}

pub fn unmap_page_table(asid: asid_t, vaddr: VPtr, pt: &PTE) {
    let find_ret = find_vspace_for_asid(asid);
    if find_ret.status != exception_t::EXCEPTION_NONE {
        return;
    }
    let mut ptSlot: *mut PTE = core::ptr::null_mut::<PTE>();
    let mut pte = find_ret.vspace_root.unwrap();
    let mut level: usize = 0;
    while level < UPT_LEVELS - 1 && pte as usize != pt as *const PTE as usize {
        ptSlot = unsafe { pte.add(VAddr(vaddr.raw()).get_upt_index(level)) };
        if ptr_to_mut(ptSlot).get_type() != (pte_tag_t::pte_table) as usize {
            return;
        }
        pte = ptr_to_mut(ptSlot)
            .next_level_paddr()
            .to_pptr()
            .get_mut_ptr::<PTE>();
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
    vptr: VPtr,
    pptr: PPtr,
) -> Result<(), lookup_fault> {
    let addr = pptr.to_paddr();
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
    assert!(asid < bit!(16));
    invalidate_tlb_by_asid(asid);
    Ok(())

    // match page_size {
    //     ARM_SMALL_PAGE => {
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
    //     ARM_LARGE_PAGE => {
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

pub fn do_flush(invLabel: MessageLabel, start: usize, end: usize, pstart: PAddr) {
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
