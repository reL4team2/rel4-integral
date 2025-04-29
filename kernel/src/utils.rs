#[cfg(target_arch = "aarch64")]
use sel4_common::{
    sel4_config::{ID_AA64PFR0_EL1_ASIMD, ID_AA64PFR0_EL1_FP},
    MASK,
	utils::ptr_to_usize_add,
};
#[cfg(target_arch = "aarch64")]
use sel4_vspace::{dsb, isb};

use crate::BIT;

#[cfg(target_arch = "riscv64")]
#[inline]
pub fn clear_memory(ptr: *mut u8, bits: usize) {
    unsafe {
        core::slice::from_raw_parts_mut(ptr, BIT!(bits)).fill(0);
    }
}

// /* Cleaning memory before user-level access */
// static inline void clearMemory(word_t *ptr, word_t bits)
// {
//     memzero(ptr, (1ul << (bits)));
//     cleanCacheRange_RAM((word_t)ptr, (word_t)ptr + (1ul << (bits)) - 1,
//                         addrFromPPtr(ptr));
// }

#[cfg(target_arch = "aarch64")]
#[inline]
pub fn clear_memory(ptr: *mut u8, bits: usize) {
    use sel4_vspace::{clean_cache_range_ram, pptr_to_paddr};

    unsafe {
        core::slice::from_raw_parts_mut(ptr, BIT!(bits)).fill(0);
        clean_cache_range_ram(
            ptr as usize,
			ptr_to_usize_add(ptr,BIT!(bits) - 1),
            pptr_to_paddr(ptr as usize),
        );
    }
}

// static inline void clearMemory_PT(word_t *ptr, word_t bits)
// {
//     memzero(ptr, (1ul << (bits)));
//     cleanCacheRange_PoU((word_t)ptr, (word_t)ptr + (1ul << (bits)) - 1,
//                         addrFromPPtr(ptr));
// }

// #[cfg(target_arch = "aarch64")]
// #[inline]
// pub fn clear_memory_pt(ptr: *mut u8, bits: usize) {
//     use sel4_vspace::{clean_cache_range_pou, pptr_to_paddr};

//     unsafe {
//         core::slice::from_raw_parts_mut(ptr, BIT!(bits)).fill(0);
//         clean_cache_range_pou(
//             ptr as usize,
//             ptr.add(BIT!(bits) - 1) as usize,
//             pptr_to_paddr(ptr as usize),
//         );
//     }
// }

#[inline]
#[cfg(target_arch = "aarch64")]
pub fn setVTable(addr: usize) {
    dsb();
    unsafe {
        core::arch::asm!("MSR vbar_el1, {0}", in(reg) addr);
    }
    isb();
}

#[inline]
#[cfg(target_arch = "aarch64")]
pub fn fpsimd_HWCapTest() -> bool {
    let mut id_aa64pfr0: usize;

    // 读取系统寄存器
    unsafe {
        core::arch::asm!("mrs {}, id_aa64pfr0_el1", out(reg) id_aa64pfr0);
    }

    // 检查硬件是否支持FP和ASIMD
    if ((id_aa64pfr0 >> ID_AA64PFR0_EL1_FP) & MASK!(4)) == MASK!(4)
        || ((id_aa64pfr0 >> ID_AA64PFR0_EL1_ASIMD) & MASK!(4)) == MASK!(4)
    {
        return false;
    }

    true
}
