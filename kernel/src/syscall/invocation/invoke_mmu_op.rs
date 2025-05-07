#[cfg(target_arch = "aarch64")]
use core::intrinsics::unlikely;
#[cfg(target_arch = "aarch64")]
use sel4_common::BIT;
use sel4_common::{
    arch::ArchReg,
    message_info::seL4_MessageInfo_func,
    shared_types_bf_gen::seL4_MessageInfo,
    structures_gen::{cap, cap_page_table_cap},
};

use sel4_common::structures_gen::cap_frame_cap;
#[cfg(target_arch = "aarch64")]
use sel4_common::utils::convert_ref_type_to_usize;
#[cfg(target_arch = "riscv64")]
use sel4_common::{
    arch::maskVMRights,
    sel4_bitfield_types::Bitfield,
    shared_types_bf_gen::seL4_CapRights,
    utils::{max_free_index, pageBitsForSize},
    MASK,
};
use sel4_common::{sel4_config::*, structures::exception_t, utils::convert_to_mut_type_ref};

#[cfg(target_arch = "riscv64")]
use sel4_cspace::interface::cte_insert;
use sel4_cspace::interface::cte_t;
use sel4_task::{get_currenct_thread, set_thread_state, ThreadState};
#[cfg(target_arch = "riscv64")]
use sel4_vspace::{
    asid_pool_t, copyGlobalMappings, pptr_t, set_asid_pool_by_index, sfence, vm_attributes_t,
    PTEFlags,
};
#[cfg(target_arch = "aarch64")]
use sel4_vspace::{clean_by_va_pou, invalidate_tlb_by_asid_va, pte_tag_t};
use sel4_vspace::{pptr_to_paddr, unmap_page, unmap_page_table, PTE};

use crate::{kernel::boot::current_lookup_fault, utils::clear_memory};

pub fn invoke_page_table_unmap(capability: &mut cap_page_table_cap) -> exception_t {
    if capability.get_capPTIsMapped() != 0 {
        let pt = convert_to_mut_type_ref::<PTE>(capability.get_capPTBasePtr() as usize);
        unmap_page_table(
            capability.get_capPTMappedASID() as usize,
            capability.get_capPTMappedAddress() as usize,
            pt,
        );
        clear_memory(pt.get_mut_ptr() as *mut u8, SEL4_PAGE_TABLE_BITS)
    }
    capability.set_capPTIsMapped(0);
    exception_t::EXCEPTION_NONE
}
#[cfg(target_arch = "riscv64")]
pub fn invoke_page_table_map(
    pt_cap: &mut cap_page_table_cap,
    pt_slot: &mut PTE,
    asid: usize,
    vaddr: usize,
) -> exception_t {
    let paddr = pptr_to_paddr(pt_cap.get_capPTBasePtr() as usize);
    let pte = PTE::new(paddr >> SEL4_PAGE_BITS, PTEFlags::V);
    *pt_slot = pte;
    pt_cap.set_capPTIsMapped(1);
    pt_cap.set_capPTMappedASID(asid as u64);
    pt_cap.set_capPTMappedAddress(vaddr as u64);
    sfence();
    exception_t::EXCEPTION_NONE
}
// #[allow(unused)]
// #[cfg(target_arch = "aarch64")]
// pub fn invoke_page_table_map(
//     pt_cap: &mut cap_t,
//     pd_slot: &mut PDE,
//     asid: usize,
//     vaddr: usize,
// ) -> exception_t {
//     let paddr = pptr_to_paddr(pt_cap.get_pt_base_ptr());
//     let pde = PDE::new_small(paddr >> SEL4_PAGE_BITS);
//     *pd_slot = pde;
//     pt_cap.set_pt_is_mapped(1);
//     pt_cap.set_pt_mapped_asid(asid);
//     pt_cap.set_pt_mapped_address(vaddr);
//     unsafe {
//         asm!(
//             "dc cvau, {}",
//             "dmb sy",
//             in(reg) pd_slot,
//         );
//     }
//     exception_t::EXCEPTION_NONE
// }

pub fn invoke_page_get_address(vbase_ptr: usize, call: bool) -> exception_t {
    let thread = get_currenct_thread();
    if call {
        thread.tcbArch.set_register(ArchReg::Badge, 0);
        let length = thread.set_mr(0, pptr_to_paddr(vbase_ptr)) as u64;
        thread.tcbArch.set_register(
            ArchReg::MsgInfo,
            seL4_MessageInfo::new(0, 0, 0, length).to_word(),
        );
    }
    set_thread_state(thread, ThreadState::ThreadStateRestart);
    exception_t::EXCEPTION_NONE
}

pub fn invoke_page_unmap(frame_slot: &mut cte_t) -> exception_t {
    if cap::cap_frame_cap(&frame_slot.capability).get_capFMappedASID() as usize != ASID_INVALID {
        match unmap_page(
            cap::cap_frame_cap(&frame_slot.capability).get_capFSize() as usize,
            cap::cap_frame_cap(&frame_slot.capability).get_capFMappedASID() as usize,
            // FIXME: here should be frame_mapped_address.
            cap::cap_frame_cap(&frame_slot.capability).get_capFMappedAddress() as usize,
            cap::cap_frame_cap(&frame_slot.capability).get_capFBasePtr() as usize,
        ) {
            Err(lookup_fault) => unsafe {
                current_lookup_fault = lookup_fault;
            },
            _ => {}
        }
    }
    cap::cap_frame_cap(&frame_slot.capability).set_capFMappedAddress(0);
    cap::cap_frame_cap(&frame_slot.capability).set_capFMappedASID(ASID_INVALID as u64);
    exception_t::EXCEPTION_NONE
}

#[cfg(target_arch = "riscv64")]
pub fn invoke_page_map(
    _frame_cap: &mut cap_frame_cap,
    w_rights_mask: usize,
    vaddr: usize,
    asid: usize,
    attr: vm_attributes_t,
    pt_slot: &mut PTE,
    frame_slot: &mut cte_t,
) -> exception_t {
    let frame_vm_rights = unsafe {
        core::mem::transmute(cap::cap_frame_cap(&frame_slot.capability).get_capFVMRights())
    };
    let vm_rights = maskVMRights(
        frame_vm_rights,
        seL4_CapRights(Bitfield {
            arr: [w_rights_mask as u64; 1],
        }),
    );
    let frame_addr =
        pptr_to_paddr(cap::cap_frame_cap(&frame_slot.capability).get_capFBasePtr() as usize);
    cap::cap_frame_cap(&frame_slot.capability).set_capFMappedAddress(vaddr as u64);
    cap::cap_frame_cap(&frame_slot.capability).set_capFMappedASID(asid as u64);
    #[cfg(target_arch = "riscv64")]
    let executable = attr.get_execute_never() == 0;
    #[cfg(target_arch = "riscv64")]
    let pte = PTE::make_user_pte(frame_addr, executable, vm_rights);
    #[cfg(target_arch = "aarch64")]
    let pte = PTE::make_user_pte(frame_addr, vm_rights, attr, frame_slot.cap.get_frame_size());
    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    pt_slot.update(pte);
    exception_t::EXCEPTION_NONE
}
#[cfg(target_arch = "aarch64")]
pub fn invoke_page_map(
    asid: usize,
    capability: cap_frame_cap,
    pte: PTE,
    pt_slot: &mut PTE,
) -> exception_t {
    let tlbflush_required: bool = pt_slot.get_type() != (pte_tag_t::pte_invalid) as usize;
    pt_slot.update(pte);

    clean_by_va_pou(
        convert_ref_type_to_usize(pt_slot),
        pptr_to_paddr(convert_ref_type_to_usize(pt_slot)),
    );
    if unlikely(tlbflush_required) {
        assert!(asid < BIT!(16));
        invalidate_tlb_by_asid_va(asid, capability.get_capFMappedAddress() as usize);
    }
    exception_t::EXCEPTION_NONE
}
// #[cfg(target_arch = "aarch64")]
// pub fn invoke_huge_page_map(
//     vaddr: usize,
//     asid: usize,
//     frame_slot: &mut cte_t,
//     pude: PUDE,
//     pudSlot: &mut PUDE,
// ) -> exception_t {
//     frame_slot.cap.set_frame_mapped_address(vaddr);
//     frame_slot.cap.set_frame_mapped_asid(asid);
//     *pudSlot = pude;
//     unsafe {
//         asm!(
//             "dc cvau, {}",
//             "dmb sy",
//             in(reg) pudSlot,
//         );
//     }
//     let tlbflush_required = pudSlot.get_pude_type() == 1;
//     if tlbflush_required {
//         assert!(asid < BIT!(16));
//         invalidate_tlb_by_asid_va(asid, vaddr);
//     }
//     exception_t::EXCEPTION_NONE
// }

// #[cfg(target_arch = "aarch64")]
// pub fn invoke_large_page_map(
//     vaddr: usize,
//     asid: usize,
//     frame_slot: &mut cte_t,
//     pde: PDE,
//     pdSlot: &mut PDE,
// ) -> exception_t {
//     frame_slot.cap.set_frame_mapped_address(vaddr);
//     frame_slot.cap.set_frame_mapped_asid(asid);
//     *pdSlot = pde;
//     unsafe {
//         asm!(
//             "dc cvau, {}",
//             "dmb sy",
//             in(reg) pdSlot,
//         );
//     }
//     let tlbflush_required = pdSlot.get_pde_type() == 1;
//     if tlbflush_required {
//         assert!(asid < BIT!(16));
//         invalidate_tlb_by_asid_va(asid, vaddr);
//     }
//     exception_t::EXCEPTION_NONE
// }

// #[cfg(target_arch = "aarch64")]
// pub fn invoke_small_page_map(
//     vaddr: usize,
//     asid: usize,
//     frame_slot: &mut cte_t,
//     pte: PTE,
//     ptSlot: &mut PTE,
// ) -> exception_t {
//     frame_slot.cap.set_frame_mapped_address(vaddr);
//     frame_slot.cap.set_frame_mapped_asid(asid);
//     *ptSlot = pte;
//     unsafe {
//         asm!(
//             "dc cvau, {}",
//             "dmb sy",
//             in(reg) ptSlot,
//         );
//     }
//     let tlbflush_required = ptSlot.is_present();
//     if tlbflush_required {
//         assert!(asid < BIT!(16));
//         invalidate_tlb_by_asid_va(asid, vaddr);
//     }
//     exception_t::EXCEPTION_NONE
// }

#[cfg(target_arch = "riscv64")]
pub fn invoke_asid_control(
    frame_ptr: pptr_t,
    slot: &mut cte_t,
    parent_slot: &mut cte_t,
    asid_base: usize,
) -> exception_t {
    use sel4_common::structures_gen::cap_asid_pool_cap;

    cap::cap_untyped_cap(&parent_slot.capability).set_capFreeIndex(max_free_index(
        cap::cap_untyped_cap(&parent_slot.capability).get_capBlockSize() as usize,
    ) as u64);
    clear_memory(frame_ptr as *mut u8, pageBitsForSize(RISCV_4K_PAGE));
    cte_insert(
        &cap_asid_pool_cap::new(asid_base as u64, frame_ptr as u64).unsplay(),
        parent_slot,
        slot,
    );
    assert_eq!(asid_base & MASK!(ASID_LOW_BITS), 0);
    set_asid_pool_by_index(asid_base >> ASID_LOW_BITS, frame_ptr);
    exception_t::EXCEPTION_NONE
}

#[cfg(target_arch = "riscv64")]
pub fn invoke_asid_pool(
    asid: usize,
    pool: &mut asid_pool_t,
    vspace_slot: &mut cte_t,
) -> exception_t {
    let region_base = cap::cap_page_table_cap(&vspace_slot.capability).get_capPTBasePtr() as usize;
    cap::cap_page_table_cap(&vspace_slot.capability).set_capPTIsMapped(1);
    cap::cap_page_table_cap(&vspace_slot.capability).set_capPTMappedAddress(0);
    cap::cap_page_table_cap(&vspace_slot.capability).set_capPTMappedASID(asid as u64);

    copyGlobalMappings(region_base);
    pool.set_vspace_by_index(asid & MASK!(ASID_LOW_BITS), region_base);
    exception_t::EXCEPTION_NONE
}
