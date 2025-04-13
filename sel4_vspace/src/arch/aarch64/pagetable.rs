use crate::{map_it_pud_cap, vptr_t, PageTable};
use sel4_common::structures::pptr_t;
use sel4_common::structures_gen::{cap_page_table_cap, cap_vspace_cap};

impl PageTable {
    pub(crate) const PTE_NUM_IN_PAGE: usize = 0x200;
}

/// Create a new pud cap in the vspace.
///
/// vptr is the virtual address of the pud cap will be created
/// pptr is the address to the physical address will be mapped
#[no_mangle]
#[link_section = ".boot.text"]
pub fn create_it_pud_cap(
    vspace_cap: &cap_vspace_cap,
    pptr: pptr_t,
    vptr: vptr_t,
    asid: usize,
) -> cap_page_table_cap {
    let capability = cap_page_table_cap::new(asid as u64, pptr as u64, 1, vptr as u64);
    map_it_pud_cap(vspace_cap, &capability);
    return capability;
}
