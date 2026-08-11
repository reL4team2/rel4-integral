use crate::PTE;
use sel4_common::{
    sel4_config::{ASID_HIGH_BITS, ASID_LOW_BITS, ASID_INVALID, IT_ASID},
    structures::exception_t,
    structures_gen::{
        asid_map_Splayed, asid_map_asid_map_none, asid_map_asid_map_vspace, asid_map_tag, cap,
        cap_asid_pool_cap, cap_vspace_cap, lookup_fault, lookup_fault_invalid_root,
    },
    utils::{convert_to_mut_type_ref, convert_to_option_mut_type_ref},
};

use crate::{asid_pool_t, asid_t, findVSpaceForASID_ret, set_vm_root};
use sel4_common::structures_gen::asid_map;

use super::asid_pool_from_addr;

/// Maximum number of hardware VMID/ASID slots.
/// ARMv8 typically supports 8-bit VMIDs (256 slots).
#[cfg(feature = "hypervisor")]
const HW_ASID_MAX: usize = bit!(8);

pub(crate) static mut armKSASIDTable: [usize; bit!(ASID_HIGH_BITS)] = [0; bit!(ASID_HIGH_BITS)];

/// Hardware VMID/ASID → software ASID mapping table (hypervisor only).
/// Indexed by hardware VMID (0..255), contains the software ASID that owns it.
#[cfg(feature = "hypervisor")]
pub(crate) static mut armKSHWASIDTable: [usize; HW_ASID_MAX] = [ASID_INVALID; HW_ASID_MAX];

/// Next hardware VMID candidate for round-robin allocation (hypervisor only).
#[cfg(feature = "hypervisor")]
pub(crate) static mut armKSNextASID: usize = 0;

// ---------------------------------------------------------------------------
// VMID management — hypervisor-only helpers
// ---------------------------------------------------------------------------

#[cfg(feature = "hypervisor")]
#[inline]
fn get_asid_map_for_asid_mut(asid: usize) -> Option<&'static mut asid_map> {
    let pool_ptr = get_asid_pool_by_index(asid >> ASID_LOW_BITS);
    if pool_ptr == 0 {
        return None;
    }
    let pool = convert_to_mut_type_ref::<asid_pool_t>(pool_ptr);
    Some(&mut pool[asid & mask_bits!(ASID_LOW_BITS)])
}

/// Mark the software ASID's hardware VMID as invalid in the asid_map.
#[cfg(feature = "hypervisor")]
pub fn invalidate_asid(asid: usize) {
    if let Some(asidmap) = get_asid_map_for_asid_mut(asid) {
        match asidmap.clone().splay() {
            asid_map_Splayed::asid_map_vspace(data) => {
                let mut new_data = data;
                new_data.set_stored_hw_vmid(0);
                new_data.set_stored_vmid_valid(0);
                *asidmap = new_data.unsplay();
            }
            _ => {}
        }
    }
}

/// Fully invalidate an ASID entry: clear HW ASID table + mark asid_map invalid.
#[cfg(feature = "hypervisor")]
pub fn invalidate_asid_entry(asid: usize) {
    let found = find_map_for_asid(asid);
    match found {
        Some(map) => match map.clone().splay() {
            asid_map_Splayed::asid_map_vspace(data) => {
                if data.get_stored_vmid_valid() != 0 {
                    let hw_asid = data.get_stored_hw_vmid() as usize;
                    unsafe {
                        armKSHWASIDTable[hw_asid] = ASID_INVALID;
                    }
                }
                invalidate_asid(asid);
            }
            _ => {}
        },
        None => {}
    }
}

/// Associate a software ASID with a hardware VMID.
#[cfg(feature = "hypervisor")]
pub fn store_hw_asid(asid: usize, hw_asid: usize) {
    if let Some(asidmap) = get_asid_map_for_asid_mut(asid) {
        match asidmap.clone().splay() {
            asid_map_Splayed::asid_map_vspace(data) => {
                let mut new_data = data;
                new_data.set_stored_hw_vmid(hw_asid as u64);
                new_data.set_stored_vmid_valid(1);
                *asidmap = new_data.unsplay();
                unsafe {
                    armKSHWASIDTable[hw_asid] = asid;
                }
            }
            _ => {}
        }
    }
}

/// Find a free hardware VMID slot, recycling the oldest if necessary.
#[cfg(feature = "hypervisor")]
pub fn find_free_hw_asid() -> usize {
    let mut hw_asid_offset: usize = 0;
    let hw_asid_max = HW_ASID_MAX;
    let start = unsafe { armKSNextASID };

    // Scan for a free slot starting from armKSNextASID
    while hw_asid_offset < hw_asid_max {
        let hw_asid = (start + hw_asid_offset) % hw_asid_max;
        if unsafe { armKSHWASIDTable[hw_asid] } == ASID_INVALID {
            return hw_asid;
        }
        hw_asid_offset += 1;
    }

    // No free slot — recycle the current armKSNextASID's entry
    let hw_asid = unsafe { armKSNextASID };
    let old_asid = unsafe { armKSHWASIDTable[hw_asid] };
    invalidate_asid(old_asid);
    // Flush all Stage-2 TLB entries tagged with this VMID
    // At EL2, TLBI ASIDE1 treats the ASID value as VMID.
    unsafe {
        core::arch::asm!("tlbi aside1is, {}", in(reg) (hw_asid << 48));
    }
    crate::dsb();
    crate::isb();
    unsafe {
        armKSHWASIDTable[hw_asid] = ASID_INVALID;
        armKSNextASID = (armKSNextASID + 1) % hw_asid_max;
    }
    hw_asid
}

/// Get (or allocate) the hardware VMID for a software ASID.
#[cfg(feature = "hypervisor")]
pub fn get_hw_asid(asid: usize) -> usize {
    let found = find_map_for_asid(asid);
    match found {
        Some(map) => match map.clone().splay() {
            asid_map_Splayed::asid_map_vspace(data) => {
                if data.get_stored_vmid_valid() != 0 {
                    return data.get_stored_hw_vmid() as usize;
                }
                // VMID not valid — allocate new
                let new_hw_asid = find_free_hw_asid();
                store_hw_asid(asid, new_hw_asid);
                new_hw_asid
            }
            _ => 0,
        },
        None => 0,
    }
}

#[inline]
fn get_asid_table() -> &'static mut [usize] {
    unsafe { core::slice::from_raw_parts_mut(&raw mut armKSASIDTable as _, bit!(ASID_HIGH_BITS)) }
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
    let poolPtr = convert_to_option_mut_type_ref::<asid_pool_t>(get_asid_pool_by_index(
        asid >> ASID_LOW_BITS,
    ));
    if let Some(pool) = poolPtr {
        return Some(&pool[asid & mask_bits!(ASID_LOW_BITS)]);
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
pub fn delete_asid(asid: usize, vspace: *mut PTE, capability: &cap) -> Result<(), lookup_fault> {
    let ptr =
        convert_to_option_mut_type_ref::<asid_pool_t>(get_asid_table()[asid >> ASID_LOW_BITS]);
    if let Some(pool) = ptr {
        let asidmap = &pool[asid & mask_bits!(ASID_LOW_BITS)];
        match asidmap.clone().splay() {
            asid_map_Splayed::asid_map_vspace(data) => {
                if data.get_vspace_root() == vspace as u64 {
                    crate::invalidate_tlb_by_asid(asid);
                    #[cfg(feature = "hypervisor")]
                    invalidate_asid_entry(asid);
                    pool[asid & mask_bits!(ASID_LOW_BITS)] =
                        asid_map_asid_map_none::new().unsplay();
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
    let pool_in_table = get_asid_pool_by_index(asid_base >> ASID_LOW_BITS);
    if pool as usize == pool_in_table {
        // clear all asid in target asid pool
        let pool = convert_to_mut_type_ref::<asid_pool_t>(pool_in_table);
        for offset in 0..bit!(ASID_LOW_BITS) {
            let asidmap = &pool[offset];
            if asidmap.get_tag() == asid_map_tag::asid_map_asid_map_vspace {
                crate::invalidate_tlb_by_asid(asid_base + offset);
                #[cfg(feature = "hypervisor")]
                invalidate_asid_entry(asid_base + offset);
            }
        }
        set_asid_pool_by_index(asid_base >> ASID_LOW_BITS, 0);
        return set_vm_root(default_vspace_cap);
    }
    Ok(())
}

#[no_mangle]
pub fn write_it_asid_pool(it_ap_cap: &cap_asid_pool_cap, it_vspace_cap: &cap_vspace_cap) {
    let ap = asid_pool_from_addr(it_ap_cap.get_capASIDPool() as usize);
    #[cfg(not(feature = "hypervisor"))]
    let asidmap = asid_map_asid_map_vspace::new(it_vspace_cap.get_capVSBasePtr() as u64).unsplay();
    #[cfg(feature = "hypervisor")]
    let asidmap = asid_map_asid_map_vspace::new(it_vspace_cap.get_capVSBasePtr() as u64, 0, 0).unsplay();
    ap[IT_ASID] = asidmap;
    set_asid_pool_by_index(IT_ASID >> ASID_LOW_BITS, ap as *const _ as usize);
}