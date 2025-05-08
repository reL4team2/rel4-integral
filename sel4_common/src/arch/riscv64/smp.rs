use crate::BIT;
use crate::ffi::kernel_stack_alloc;
use core::arch::asm;
use crate::sel4_config::{CONFIG_KERNEL_STACK_BITS, CONFIG_MAX_NUM_NODES};

#[derive(Clone, Copy)]
pub struct core_map_t {
    pub map: [usize; CONFIG_MAX_NUM_NODES],
}

static mut core_map: core_map_t = core_map_t {
    map: [0; CONFIG_MAX_NUM_NODES],
};

#[inline]
fn get_core_map_ref() -> &'static [usize; CONFIG_MAX_NUM_NODES] {
    unsafe { &core_map.map }
}

#[inline]
pub fn cpu_index_to_id(index: usize) -> usize {
    assert!(index < CONFIG_MAX_NUM_NODES);
    unsafe { get_core_map_ref()[index] }
}

#[inline]
pub fn get_current_cpu_index() -> usize {
    unsafe {
        let mut cur_sp: usize;
        asm!(
        "csrr {}, sscratch",
        out(reg) cur_sp,
        );
        cur_sp -= unsafe { &kernel_stack_alloc.data[0][0] as *const u8 } as usize + 8;
        cur_sp >> CONFIG_KERNEL_STACK_BITS
    }
}

#[inline]
pub fn hart_id_to_core_id(hart_id: usize) -> usize {
    unsafe {
        match get_core_map_ref().iter().position(|&x| x == hart_id) {
            Some(core_id) => core_id,
            _ => 0,
        }
    }
}

#[inline]
pub fn get_sbi_mask_for_all_remote_harts() -> usize {
    let mut mask: usize = 0;
    for i in 0..CONFIG_MAX_NUM_NODES {
        if i != get_current_cpu_index() {
            mask |= BIT!(cpu_index_to_id(i));
        }
    }
    mask
}

pub fn add_hart_to_core_map(hart_id: usize, core_id: usize) {
    unsafe {
        assert!(core_id < CONFIG_MAX_NUM_NODES);
        core_map.map[core_id] = hart_id;
    }
}