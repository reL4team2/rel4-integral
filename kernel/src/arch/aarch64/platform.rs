use crate::arch::aarch64::fpu::disable_fpu;
use aarch64_cpu::registers::TPIDR_EL1;
use aarch64_cpu::registers::{Writeable, CNTKCTL_EL1};
use core::arch::asm;
use sel4_common::arch::config::{KERNEL_ELF_BASE, PADDR_TOP};
use sel4_common::ffi::kernel_stack_alloc;
use sel4_common::ffi_addr;
use sel4_common::platform::{timer, Timer_func};
use sel4_common::sel4_config::*;
use sel4_common::structures::p_region_t;
use sel4_common::utils::cpu_id;

use crate::boot::{
    avail_p_regs_addr, avail_p_regs_size, paddr_to_pptr_reg, res_reg, reserve_region,
    rust_init_freemem,
};
use crate::utils::{fpsime_hw_cap_test, set_vtable};
use log::debug;
use sel4_vspace::*;

use super::arm_gic::gic_v2::gic_v2::{cpu_init_local_irq_controller, dist_init};

#[allow(unused)]
pub fn init_cpu() -> bool {
    activate_kernel_vspace();

    // CPU's exception vector table
    unsafe {
        set_vtable(ffi_addr!(arm_vector_table));
    }

    // Setup kernel stack pointer.
    let mut stack_top = unsafe {
        &mut kernel_stack_alloc.data[cpu_id()] as *mut u8 as usize
            + sel4_common::BIT!(CONFIG_KERNEL_STACK_BITS)
    };

    #[cfg(feature = "enable_smp")]
    {
        stack_top |= cpu_id()
    }

    // CPU's exception vector table
    unsafe {
        set_vtable(ffi_addr!(arm_vector_table));
    }
    TPIDR_EL1.set(stack_top as u64);

    let haveHWFPU = fpsime_hw_cap_test();

    if haveHWFPU {
        unsafe {
            disable_fpu();
        }
    }

    // initLocalIRQController
    cpu_init_local_irq_controller();

    // armv_init_user_access
    armv_init_user_access();

    unsafe {
        timer.init_timer();
    }
    true
}

pub fn init_freemem(ui_p_reg: p_region_t, dtb_p_reg: p_region_t) -> bool {
    unsafe {
        res_reg[0].start = paddr_to_pptr(kpptr_to_paddr(KERNEL_ELF_BASE));
        res_reg[0].end = paddr_to_pptr(kpptr_to_paddr(ffi_addr!(ki_end)));
    }

    let mut index = 1;

    if dtb_p_reg.start != 0 {
        if index >= NUM_RESERVED_REGIONS {
            debug!("ERROR: no slot to add DTB to reserved regions\n");
            return false;
        }
        unsafe {
            res_reg[index] = paddr_to_pptr_reg(&dtb_p_reg);
            index += 1;
        }
    }

    // here use the MODE_RESERVED:ARRAY_SIZE(mode_reserved_region) to judge
    // but in aarch64, the array size is always 0
    // so eliminate some code
    if ui_p_reg.start < PADDR_TOP {
        if index >= NUM_RESERVED_REGIONS {
            debug!("ERROR: no slot to add the user image to the reserved regions");
            return false;
        }
        unsafe {
            // FIXED: here should be ui_p_reg, but before is dtb_p_reg.
            res_reg[index] = paddr_to_pptr_reg(&ui_p_reg);
            index += 1;
        }
    } else {
        unsafe {
            reserve_region(p_region_t {
                start: ui_p_reg.start,
                end: ui_p_reg.end,
            });
        }
    }

    unsafe { rust_init_freemem(avail_p_regs_size, avail_p_regs_addr, index, res_reg.clone()) }
}

pub fn clean_invalidate_l1_caches() {
    unsafe {
        asm!("dsb sy;"); // DSB SY
        clean_invalidate_d_pos();
        asm!("dsb sy;"); // DSB SY
        invalidate_i_pou();
        asm!("dsb sy;"); // DSB SY
    }
}
pub fn invalidate_local_tlb() {
    unsafe {
        asm!("dsb sy;"); // DSB SY
        asm!("tlbi vmalle1;");
        asm!("dsb sy;"); // DSB SY
        asm!("isb;"); // ISB SY
    }
}

fn clean_invalidate_d_pos() {
    let clid = read_clid();
    let loc = (clid >> 24) & (1 << 3 - 1);
    for l in 0..loc {
        if ((clid >> l * 3) & ((1 << 3) - 1)) > 1 {
            clean_invalidate_d_by_level(l);
        }
    }
}

#[inline]
fn clean_invalidate_d_by_level(level: usize) {
    let lsize = read_cache_size(level);
    let lbits = (lsize & (1 << 3 - 1)) + 4;
    let assoc = ((lsize >> 3) & (1 << 10 - 1)) + 1;
    let assoc_bits = WORD_BITS - (assoc - 1).leading_zeros() as usize;
    let nsets = ((lsize >> 13) & (1 << 15 - 1)) + 1;

    for w in 0..assoc {
        for s in 0..nsets {
            let wsl = (w << (32 - assoc_bits)) | (s << lbits) | (level << 1);
            unsafe {
                asm!(
                    "dc cisw, {}",
                    in(reg) wsl,
                )
            }
        }
    }
}

fn invalidate_i_pou() {
    unsafe {
        asm!("ic iallu;");
        asm!("isb;");
    }
}
fn read_clid() -> usize {
    let mut clid: usize;
    unsafe {
        asm!(
            "mrs {},clidr_el1",
            out(reg) clid,
        );
    }
    clid
}

fn read_cache_size(level: usize) -> usize {
    let mut size: usize;
    let mut csselr_old: usize;
    unsafe {
        asm!(
            "mrs {},csselr_el1",
            out(reg) csselr_old,
        );
        asm!(
            "msr csselr_el1,{}",
            in(reg) ((level << 1) | csselr_old),
        );
        asm!(
            "mrs {},csselr_el1",
            out(reg) size,
        );
        asm!(
            "msr csselr_el1,{}",
            in(reg) csselr_old,
        );
    }
    size
}
#[allow(unused_mut)]
fn armv_init_user_access() {
    let mut val: usize = 0;
    val = sel4_common::BIT!(0);

    #[cfg(feature = "enable_benchmark")]
    {
        unsafe {
            asm!(
                "msr pmuserenr_el0,{}",
                in(reg) val as usize,
            );
        }
    }

    val = 0;
    #[cfg(feature = "enable_arm_pcnt")]
    {
        val |= sel4_common::BIT!(0);
    }
    #[cfg(feature = "enable_arm_ptmr")]
    {
        val |= sel4_common::BIT!(9);
    }
    CNTKCTL_EL1.set(val as u64);
}

pub fn init_irq_controller() {
    dist_init();
}
