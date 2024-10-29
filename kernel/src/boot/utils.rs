use super::ndks_boot;
#[cfg(target_arch = "aarch64")]
use super::PAGE_SIZE_BITS;
use crate::config::CONFIG_ROOT_CNODE_SIZE_BITS;
use crate::structures::{p_region_t, region_t, v_region_t};
use crate::{BIT, ROUND_DOWN, ROUND_UP};
use log::debug;
use sel4_common::arch::config::{PADDR_TOP, PPTR_BASE, PPTR_TOP};
use sel4_common::sel4_config::*;
use sel4_common::structures_gen::{cap, cap_cnode_cap};
use sel4_cspace::interface::*;
use sel4_vspace::*;
// #[cfg(target_arch="riscv64")]
// use sel4_vspace::

#[inline]
pub fn is_reg_empty(reg: &region_t) -> bool {
    reg.start == reg.end
}

#[inline]
pub fn paddr_to_pptr_reg(reg: &p_region_t) -> region_t {
    region_t {
        start: paddr_to_pptr(reg.start),
        end: paddr_to_pptr(reg.end),
    }
}

pub fn ceiling_kernel_window(mut p: usize) -> usize {
    if pptr_to_paddr(p) > PADDR_TOP {
        p = PPTR_TOP;
    }
    p
}

#[inline]
pub fn pptr_to_paddr_reg(reg: region_t) -> p_region_t {
    p_region_t {
        start: pptr_to_paddr(reg.start),
        end: pptr_to_paddr(reg.end),
    }
}

pub fn pptr_in_kernel_window(pptr: usize) -> bool {
    pptr >= PPTR_BASE && pptr < PPTR_TOP
}

#[inline]
pub fn get_n_paging(v_reg: v_region_t, bits: usize) -> usize {
    let start = ROUND_DOWN!(v_reg.start, bits);
    let end = ROUND_UP!(v_reg.end, bits);
    (end - start) / BIT!(bits)
}

#[cfg(target_arch = "riscv64")]
pub fn arch_get_n_paging(it_v_reg: v_region_t) -> usize {
    let mut n: usize = 0;
    for i in 0..CONFIG_PT_LEVELS - 1 {
        n += get_n_paging(it_v_reg, RISCV_GET_LVL_PGSIZE_BITS(i));
    }
    n
}

#[cfg(target_arch = "aarch64")]
pub fn arch_get_n_paging(it_v_reg: v_region_t) -> usize {
    let n = get_n_paging(it_v_reg, 3 * PT_INDEX_BITS + PAGE_SIZE_BITS)
        + get_n_paging(it_v_reg, PT_INDEX_BITS + PAGE_SIZE_BITS + PT_INDEX_BITS)
        + get_n_paging(it_v_reg, PT_INDEX_BITS + PAGE_SIZE_BITS);
    n
}

pub fn write_slot(ptr: *mut cte_t, capability: cap) {
    unsafe {
        (*ptr).capability = capability;
        (*ptr).cteMDBNode = mdb_node_t::default();
        let mdb = &mut (*ptr).cteMDBNode;
        mdb.set_revocable(1);
        mdb.set_first_badged(1);
    }
}

pub fn provide_cap(root_cnode_cap: &cap_cnode_cap, capability: cap) -> bool {
    unsafe {
        if ndks_boot.slot_pos_cur >= BIT!(CONFIG_ROOT_CNODE_SIZE_BITS) {
            debug!(
                "ERROR: can't add another cap, all {} (=2^CONFIG_ROOT_CNODE_SIZE_BITS) slots used",
                BIT!(CONFIG_ROOT_CNODE_SIZE_BITS)
            );
            return false;
        }
        let ptr = root_cnode_cap.get_capCNodePtr() as *mut cte_t;
        write_slot(ptr.add(ndks_boot.slot_pos_cur), capability);
        ndks_boot.slot_pos_cur += 1;
        return true;
    }
}
