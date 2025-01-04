use crate::PTE;
use sel4_common::{
    sel4_config::{asidHighBits, asidLowBits, IT_ASID},
    structures::exception_t,
    structures_gen::{
        asid_map_Splayed, asid_map_asid_map_none, asid_map_asid_map_vspace, asid_map_tag, cap, cap_asid_pool_cap, cap_vspace_cap, lookup_fault, lookup_fault_invalid_root
    },
    utils::{convert_to_mut_type_ref, convert_to_option_mut_type_ref},
    BIT, MASK,
};

use crate::{asid_pool_t, asid_t, findVSpaceForASID_ret, set_vm_root};
use sel4_common::structures_gen::asid_map;

use super::asid_pool_from_addr;
use super::machine::invalidate_local_tlb_asid;

pub(crate) static mut armKSASIDTable: [usize; BIT!(asidHighBits)] = [0; BIT!(asidHighBits)];

#[inline]
fn get_asid_table() -> &'static mut [usize] {
    unsafe { core::slice::from_raw_parts_mut(armKSASIDTable.as_mut_ptr(), BIT!(asidHighBits)) }
}

#[inline]
pub fn get_asid_pool_by_index(idx: usize) -> usize {
    unsafe { armKSASIDTable[idx] }
}

#[inline]
pub fn set_asid_pool_by_index(idx: usize, val: usize) {
    unsafe {
        armKSASIDTable[idx] = val;
    }
}

#[no_mangle]
pub fn find_map_for_asid(asid: usize) -> Option<&'static asid_map> {
    let poolPtr =
        convert_to_option_mut_type_ref::<asid_pool_t>(get_asid_pool_by_index(asid >> asidLowBits));
    if let Some(pool) = poolPtr {
        return Some(&pool[asid & MASK!(asidLowBits)]);
    }
    None
}

#[no_mangle]
pub fn find_vspace_for_asid(asid: usize) -> findVSpaceForASID_ret {
    let mut ret: findVSpaceForASID_ret = findVSpaceForASID_ret {
        status: exception_t::EXCEPTION_LOOKUP_FAULT,
        vspace_root: None,
        lookup_fault: Some(lookup_fault_invalid_root::new().unsplay()),
    };
    match find_map_for_asid(asid) {
        Some(asidmap) => match asidmap.clone().splay() {
            asid_map_Splayed::asid_map_vspace(data) => {
                ret.vspace_root = Some(data.get_vspace_root() as *mut PTE);
                ret.status = exception_t::EXCEPTION_NONE;
            }
            _ => {}
        },
        None => {}
    }
    ret
}

#[no_mangle]
pub fn delete_asid(
    asid: usize,
    vspace: *mut PTE,
    capability: &cap,
) -> Result<(), lookup_fault> {
    let ptr = convert_to_option_mut_type_ref::<asid_pool_t>(get_asid_table()[asid >> asidLowBits]);
    if let Some(pool) = ptr {
        let asidmap = &pool[asid & MASK!(asidLowBits)];
        match asidmap.clone().splay() {
            asid_map_Splayed::asid_map_vspace(data) => {
                if data.get_vspace_root() == vspace as u64 {
                    invalidate_local_tlb_asid(asid);
                    pool[asid & MASK!(asidLowBits)] = asid_map_asid_map_none::new().unsplay();
                    return set_vm_root(capability);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[no_mangle]
pub fn delete_asid_pool(
    asid_base: asid_t,
    pool: *mut asid_pool_t,
    default_vspace_cap: &cap,
) -> Result<(), lookup_fault> {
    let pool_in_table = get_asid_pool_by_index(asid_base >> asidLowBits);
    if pool as usize == pool_in_table {
        // clear all asid in target asid pool
        let pool = convert_to_mut_type_ref::<asid_pool_t>(pool_in_table);
        for offset in 0..BIT!(asidLowBits) {
            let asidmap = &pool[offset];
            if asidmap.get_tag() == asid_map_tag::asid_map_asid_map_vspace {
                invalidate_local_tlb_asid(asid_base + offset);
            }
        }
        set_asid_pool_by_index(asid_base >> asidLowBits, 0);
        return set_vm_root(default_vspace_cap);
    }
    Ok(())
}

#[no_mangle]
#[inline]
pub fn write_it_asid_pool(it_ap_cap: &cap_asid_pool_cap, it_vspace_cap: &cap_vspace_cap) {
    let ap = asid_pool_from_addr(it_ap_cap.get_capASIDPool() as usize);
    let asidmap = asid_map_asid_map_vspace::new(it_vspace_cap.get_capVSBasePtr() as u64).unsplay();
    ap[IT_ASID] = asidmap;
    set_asid_pool_by_index(IT_ASID >> asidLowBits, ap as *const _ as usize);
}
