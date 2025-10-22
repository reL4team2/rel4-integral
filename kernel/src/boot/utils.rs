use super::ndks_boot;
use log::debug;
#[cfg(target_arch = "riscv64")]
use rel4_arch::basic::VRegion;
#[cfg(target_arch = "aarch64")]
use rel4_arch::basic::VRegion;
use sel4_common::arch::config::{PADDR_TOP, PPTR_BASE, PPTR_TOP};
use sel4_common::sel4_bitfield_types::Bitfield;
use sel4_common::sel4_config::*;
use sel4_common::structures_gen::{cap, cap_cnode_cap, mdb_node};
use sel4_cspace::interface::*;
#[cfg(target_arch = "riscv64")]
use sel4_vspace::riscv_get_lvl_pgsize_bits;
// use sel4_vspace::

pub fn ceiling_kernel_window(mut p: usize) -> usize {
    if pptr!(p).to_paddr().raw() > PADDR_TOP {
        p = PPTR_TOP;
    }
    p
}

pub fn pptr_in_kernel_window(pptr: usize) -> bool {
    pptr >= PPTR_BASE && pptr < PPTR_TOP
}

#[inline]
pub fn get_n_paging(v_reg: VRegion, bits: usize) -> usize {
    let start = round_down!(v_reg.start.raw(), bits);
    let end = round_up!(v_reg.end.raw(), bits);
    (end - start) / bit!(bits)
}

#[cfg(target_arch = "riscv64")]
pub fn arch_get_n_paging(it_v_reg: VRegion) -> usize {
    let mut n: usize = 0;
    for i in 0..CONFIG_PT_LEVELS - 1 {
        n += get_n_paging(it_v_reg, riscv_get_lvl_pgsize_bits(i));
    }
    n
}

#[cfg(target_arch = "aarch64")]
pub fn arch_get_n_paging(it_v_reg: VRegion) -> usize {
    let n = get_n_paging(it_v_reg, 3 * PT_INDEX_BITS + PAGE_SIZE_BITS)
        + get_n_paging(it_v_reg, PT_INDEX_BITS + PAGE_SIZE_BITS + PT_INDEX_BITS)
        + get_n_paging(it_v_reg, PT_INDEX_BITS + PAGE_SIZE_BITS);
    n
}

pub fn write_slot(ptr: *mut cte_t, capability: cap) {
    unsafe {
        (*ptr).capability = capability;
        (*ptr).cteMDBNode = mdb_node {
            0: Bitfield { arr: [0; 2usize] },
        };
        let mdb = &mut (*ptr).cteMDBNode;
        mdb.set_mdbRevocable(1);
        mdb.set_mdbFirstBadged(1);
    }
}

pub fn provide_cap(root_cnode_cap: &cap_cnode_cap, capability: cap) -> bool {
    unsafe {
        if ndks_boot.slot_pos_cur >= bit!(CONFIG_ROOT_CNODE_SIZE_BITS) {
            debug!(
                "ERROR: can't add another cap, all {} (=2^CONFIG_ROOT_CNODE_SIZE_BITS) slots used",
                bit!(CONFIG_ROOT_CNODE_SIZE_BITS)
            );
            return false;
        }
        let ptr = root_cnode_cap.get_capCNodePtr() as *mut cte_t;
        write_slot(ptr.add(ndks_boot.slot_pos_cur), capability);
        ndks_boot.slot_pos_cur += 1;
        return true;
    }
}
