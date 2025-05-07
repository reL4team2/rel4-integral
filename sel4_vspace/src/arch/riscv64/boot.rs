use super::utils::riscv_get_LVL_PGSIZE_BITS;
use crate::arch::riscv64::pagetable::{KERNEL_LEVEL2_PAGE_TABLE, KERNEL_ROOT_PAGE_TABLE};
use crate::{pptr_t, pptr_to_paddr, riscv_get_pt_index, sfence, PTEFlags, PTE};
use sel4_common::structures_gen::{cap_frame_cap, cap_page_table_cap};
use sel4_common::{
    arch::config::KDEV_BASE,
    arch::vm_rights_t,
    sel4_config::{RISCV_MEGA_PAGE_BITS, RISCV_PAGE_BITS, SEL4_PAGE_BITS},
    utils::convert_to_mut_type_ref,
    ROUND_DOWN,
};

#[no_mangle]
#[link_section = ".boot.text"]
pub fn map_kernel_frame(paddr: usize, vaddr: usize, _vm_rights: vm_rights_t) {
    if vaddr >= KDEV_BASE {
        let paddr = ROUND_DOWN!(paddr, riscv_get_LVL_PGSIZE_BITS(1));
        unsafe {
            KERNEL_LEVEL2_PAGE_TABLE.map_next_table(riscv_get_pt_index(vaddr, 0), paddr, true);
        }
    } else {
        let paddr = ROUND_DOWN!(paddr, riscv_get_LVL_PGSIZE_BITS(0));
        unsafe {
            KERNEL_ROOT_PAGE_TABLE.map_next_table(riscv_get_pt_index(vaddr, 0), paddr, true);
        }
    }
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn map_it_pt_cap(_vspace_cap: &cap_page_table_cap, _pt_cap: &cap_page_table_cap) {
    let vptr = _pt_cap.get_capPTMappedAddress() as usize;
    let lvl1pt = convert_to_mut_type_ref::<PTE>(_vspace_cap.get_capPTBasePtr() as usize);
    let pt = _pt_cap.get_capPTBasePtr() as usize;
    let pt_ret = lvl1pt.lookup_pt_slot(vptr);
    let targetSlot = convert_to_mut_type_ref::<PTE>(pt_ret.ptSlot as usize);
    *targetSlot = PTE::new(pptr_to_paddr(pt) >> SEL4_PAGE_BITS, PTEFlags::V);
    sfence();
}

#[no_mangle]
pub fn map_it_frame_cap(_vspace_cap: &cap_page_table_cap, _frame_cap: &cap_frame_cap) {
    let vptr = _frame_cap.get_capFMappedAddress() as usize;
    let lvl1pt = convert_to_mut_type_ref::<PTE>(_vspace_cap.get_capPTBasePtr() as usize);
    let frame_pptr = _frame_cap.get_capFBasePtr() as usize;
    let pt_ret = lvl1pt.lookup_pt_slot(vptr);

    let targetSlot = convert_to_mut_type_ref::<PTE>(pt_ret.ptSlot as usize);

    *targetSlot = PTE::new(
        pptr_to_paddr(frame_pptr) >> SEL4_PAGE_BITS,
        PTEFlags::ADUVRWX,
    );
    sfence();
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn create_mapped_it_frame_cap(
    pd_cap: &cap_page_table_cap,
    pptr: usize,
    vptr: usize,
    asid: usize,
    use_large: bool,
    _exec: bool,
) -> cap_frame_cap {
    let frame_size: usize;
    if use_large {
        frame_size = RISCV_MEGA_PAGE_BITS;
    } else {
        frame_size = RISCV_PAGE_BITS;
    }
    let capability = cap_frame_cap::new(
        asid as u64,
        pptr as u64,
        frame_size as u64,
        vm_rights_t::VMReadWrite as u64,
        0,
        vptr as u64,
    );
    map_it_frame_cap(pd_cap, &capability);
    capability
}

pub fn create_unmapped_it_frame_cap(pptr: pptr_t, _use_large: bool) -> cap_frame_cap {
    cap_frame_cap::new(0, pptr as u64, 0, 0, 0, 0)
}
