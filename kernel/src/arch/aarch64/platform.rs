use crate::arch::aarch64::fpu::disable_fpu;
use aarch64_cpu::asm::barrier::{self, dsb, isb};
use aarch64_cpu::registers::{self, Writeable, CNTKCTL_EL1};
use core::arch::asm;
use rel4_arch::basic::PRegion;
use sel4_common::arch::config::{KERNEL_ELF_BASE, PADDR_TOP};
use sel4_common::ffi::kernel_stack_alloc;
use sel4_common::ffi_addr;
use sel4_common::platform::{timer, Timer_func};
use sel4_common::sel4_config::*;
use sel4_common::utils::cpu_id;

use crate::boot::{
    avail_p_regs_addr, avail_p_regs_size, res_reg, reserve_region, rust_init_freemem,
};
use crate::utils::{fpsime_hw_cap_test, set_vtable};
use log::debug;
use sel4_vspace::*;

use super::arm_gic::gic_v2::gic_v2::{cpu_init_local_irq_controller, dist_init};

pub fn init_cpu() -> bool {
    activate_kernel_vspace();

    #[cfg(feature = "hypervisor")]
    super::vcpu::vcpu_boot_init();

    // CPU's exception vector table
    set_vtable(ffi_addr!(arm_vector_table));

    // Setup kernel stack pointer.
    let mut stack_top = kernel_stack_alloc.get_stack_top(cpu_id());

    #[cfg(feature = "enable_smp")]
    {
        stack_top |= cpu_id();
    }

    // CPU's exception vector table
    set_vtable(ffi_addr!(arm_vector_table));

    #[cfg(not(feature = "hypervisor"))]
    registers::TPIDR_EL1.set(stack_top as u64);
    #[cfg(feature = "hypervisor")]
    registers::TPIDR_EL2.set(stack_top as _);

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

    timer.init_timer();

    true
}

pub fn init_freemem(ui_p_reg: PRegion, dtb_p_reg: PRegion) -> bool {
    unsafe {
        res_reg[0].start = kpptr_to_paddr(KERNEL_ELF_BASE).to_pptr();
        res_reg[0].end = kpptr_to_paddr(ffi_addr!(ki_end)).to_pptr();
    }

    let mut index = 1;

    if !dtb_p_reg.start.is_null() {
        if index >= NUM_RESERVED_REGIONS {
            debug!("ERROR: no slot to add DTB to reserved regions\n");
            return false;
        }
        unsafe {
            res_reg[index] = dtb_p_reg.to_region();
            index += 1;
        }
    }

    // here use the MODE_RESERVED:ARRAY_SIZE(mode_reserved_region) to judge
    // but in aarch64, the array size is always 0
    // so eliminate some code
    if ui_p_reg.start.raw() < PADDR_TOP {
        if index >= NUM_RESERVED_REGIONS {
            debug!("ERROR: no slot to add the user image to the reserved regions");
            return false;
        }
        unsafe {
            // FIXED: here should be ui_p_reg, but before is dtb_p_reg.
            res_reg[index] = ui_p_reg.to_region();
            index += 1;
        }
    } else {
        unsafe {
            reserve_region(ui_p_reg);
        }
    }

    unsafe { rust_init_freemem(avail_p_regs_size, avail_p_regs_addr, index, res_reg.clone()) }
}

pub fn clean_invalidate_l1_caches() {
    dsb(barrier::SY);
    clean_invalidate_d_pos();
    dsb(barrier::SY);
    invalidate_i_pou();
    dsb(barrier::SY);
}
pub fn invalidate_local_tlb() {
    dsb(barrier::SY);
    unsafe { asm!("tlbi vmalle1") };
    dsb(barrier::SY);
    isb(barrier::SY);
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
    unsafe { asm!("ic iallu") };
    isb(barrier::SY);
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
    val = bit!(0);

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
        val |= bit!(0);
    }
    #[cfg(feature = "enable_arm_ptmr")]
    {
        val |= bit!(9);
    }
    CNTKCTL_EL1.set(val as u64);
}

pub fn init_irq_controller() {
    dist_init();
}
