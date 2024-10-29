use sel4_common::structures_gen::cap_page_table_cap;

use crate::{map_it_pt_cap, pptr_t, vptr_t};
#[cfg(target_arch = "aarch64")]
use sel4_common::structures_gen::cap_vspace_cap;

#[no_mangle]
#[link_section = ".boot.text"]
#[cfg(target_arch = "aarch64")]
pub fn create_it_pt_cap(
    vspace_cap: &cap_vspace_cap,
    pptr: pptr_t,
    vptr: vptr_t,
    asid: usize,
) -> cap_page_table_cap {
    let capability = cap_page_table_cap::new(asid as u64, pptr as u64, 1, vptr as u64);
    map_it_pt_cap(vspace_cap, &capability);
    return capability;
}
#[no_mangle]
#[link_section = ".boot.text"]
#[cfg(target_arch = "riscv64")]
pub fn create_it_pt_cap(
    vspace_cap: &cap_page_table_cap,
    pptr: pptr_t,
    vptr: vptr_t,
    asid: usize,
) -> cap_page_table_cap {
    let capability = cap_page_table_cap::new(asid as u64, pptr as u64, 1, vptr as u64);
    map_it_pt_cap(vspace_cap, &capability);
    return capability;
}
