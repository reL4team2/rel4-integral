use crate::arch::riscv64::pagetable::KERNEL_ROOT_PAGE_TABLE;
use crate::asid_t;
use crate::find_vspace_for_asid;
use crate::riscv_get_pt_index;
use crate::sfence;
use crate::PTEFlags;
use core::intrinsics::unlikely;
use rel4_arch::basic::VPtr;
use sel4_common::sel4_config::CONFIG_PT_LEVELS;
use sel4_common::structures_gen::cap;
use sel4_common::structures_gen::cap_tag;
use sel4_common::{
    structures::exception_t, structures_gen::lookup_fault, utils::convert_to_mut_type_ref,
};

use crate::PTE;

use super::{kpptr_to_paddr, set_vspace_root};

///根据给定的`vspace_root`设置相应的页表，会检查`vspace_root`是否合法，如果不合法默认设置为内核页表
///
/// Use page table in vspace_root to set the satp register.
pub fn set_vm_root(vspace_root_cap: &cap) -> Result<(), lookup_fault> {
    if vspace_root_cap.clone().get_tag() != cap_tag::cap_page_table_cap {
        set_vspace_root(kpptr_to_paddr(KERNEL_ROOT_PAGE_TABLE.as_ptr() as usize), 0);
        return Ok(());
    }
    let vspace_root = cap::cap_page_table_cap(vspace_root_cap);
    let lvl1pt = convert_to_mut_type_ref::<PTE>(vspace_root.get_capPTBasePtr() as usize);
    let asid = vspace_root.get_capPTMappedASID() as usize;
    let find_ret = find_vspace_for_asid(asid);
    let mut ret = Ok(());
    if unlikely(
        find_ret.status != exception_t::EXCEPTION_NONE
            || find_ret.vspace_root.is_none()
            || find_ret.vspace_root.unwrap() != lvl1pt,
    ) {
        if let Some(lookupfault) = find_ret.lookup_fault {
            ret = Err(lookupfault);
        }
        set_vspace_root(kpptr_to_paddr(KERNEL_ROOT_PAGE_TABLE.as_ptr() as usize), 0);
    }
    set_vspace_root(pptr!(lvl1pt as *mut PTE).to_paddr(), asid);
    ret
}
pub fn unmap_page_table(asid: asid_t, vptr: VPtr, pt: &mut PTE) {
    let target_pt = pt as *mut PTE;
    let find_ret = find_vspace_for_asid(asid);
    if find_ret.status != exception_t::EXCEPTION_NONE {
        return;
    }
    assert_ne!(find_ret.vspace_root.unwrap(), target_pt);
    let mut pt = find_ret.vspace_root.unwrap();
    let mut ptSlot = unsafe { &mut *(pt.add(riscv_get_pt_index(vptr.raw(), 0))) };
    let mut i = 0;
    while i < CONFIG_PT_LEVELS - 1 && pt != target_pt {
        ptSlot = unsafe { &mut *(pt.add(riscv_get_pt_index(vptr.raw(), i))) };
        if unlikely(ptSlot.is_pte_table()) {
            return;
        }
        pt = ptSlot.get_pte_from_ppn_mut() as *mut PTE;
        i += 1;
    }

    if pt != target_pt {
        return;
    }
    *ptSlot = PTE::new(0, PTEFlags::empty());
    sfence();
}
