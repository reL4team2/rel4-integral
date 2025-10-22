use super::utils::riscv_get_lvl_pgsize_bits;
use crate::arch::riscv64::pagetable::KERNEL_ROOT_PAGE_TABLE;
use crate::{riscv_get_pt_index, sfence, PTEFlags, PTE};
use rel4_arch::basic::{PAddr, PPtr, VPtr};
use sel4_common::structures_gen::{cap_frame_cap, cap_page_table_cap};
use sel4_common::{
    arch::config::KDEV_BASE,
    arch::vm_rights_t,
    sel4_config::{RISCV_MEGA_PAGE_BITS, RISCV_PAGE_BITS, SEL4_PAGE_BITS},
    utils::convert_to_mut_type_ref,
};

#[no_mangle]
#[link_section = ".boot.text"]
pub fn map_kernel_frame(paddr: PAddr, vaddr: usize, _vm_rights: vm_rights_t) {
    if vaddr >= KDEV_BASE {
        let paddr = paddr.align_down(riscv_get_lvl_pgsize_bits(1));
        KERNEL_ROOT_PAGE_TABLE.no_lock()[riscv_get_pt_index(vaddr, 0)] =
            PTE::pte_next_table(paddr, true);
    } else {
        let paddr = paddr.align_down(riscv_get_lvl_pgsize_bits(0));
        KERNEL_ROOT_PAGE_TABLE.no_lock()[riscv_get_pt_index(vaddr, 0)] =
            PTE::pte_next_table(paddr, true);
    }
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn map_it_pt_cap(vspace_cap: &cap_page_table_cap, pt_cap: &cap_page_table_cap) {
    let vptr = vptr!(pt_cap.get_capPTMappedAddress());
    let lvl1pt = convert_to_mut_type_ref::<PTE>(vspace_cap.get_capPTBasePtr() as usize);
    let pt = pptr!(pt_cap.get_capPTBasePtr());
    let pt_ret = lvl1pt.lookup_pt_slot(vptr);
    let targetSlot = convert_to_mut_type_ref::<PTE>(pt_ret.ptSlot as usize);
    *targetSlot = PTE::new(pt.to_paddr().raw() >> SEL4_PAGE_BITS, PTEFlags::V);
    sfence();
}

#[no_mangle]
pub fn map_it_frame_cap(vspace_cap: &cap_page_table_cap, frame_cap: &cap_frame_cap) {
    let vptr = vptr!(frame_cap.get_capFMappedAddress());
    let lvl1pt = convert_to_mut_type_ref::<PTE>(vspace_cap.get_capPTBasePtr() as usize);
    let frame_pptr = pptr!(frame_cap.get_capFBasePtr());
    let pt_ret = lvl1pt.lookup_pt_slot(vptr);

    let targetSlot = convert_to_mut_type_ref::<PTE>(pt_ret.ptSlot as usize);

    *targetSlot = PTE::new(
        frame_pptr.to_paddr().raw() >> SEL4_PAGE_BITS,
        PTEFlags::ADUVRWX,
    );
    sfence();
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn create_mapped_it_frame_cap(
    pd_cap: &cap_page_table_cap,
    pptr: PPtr,
    vptr: VPtr,
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
        pptr.as_u64(),
        frame_size as u64,
        vm_rights_t::VMReadWrite as u64,
        0,
        vptr.as_u64(),
    );
    map_it_frame_cap(pd_cap, &capability);
    capability
}

pub fn create_unmapped_it_frame_cap(pptr: PPtr, _use_large: bool) -> cap_frame_cap {
    cap_frame_cap::new(0, pptr.raw() as _, 0, 0, 0, 0)
}
