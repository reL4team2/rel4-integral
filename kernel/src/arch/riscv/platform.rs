use crate::arch::fpu::{init_fpu, set_fs_off};
use crate::boot::paddr_to_pptr_reg;
use crate::boot::rust_init_freemem;
use crate::boot::{avail_p_regs_addr, avail_p_regs_size, res_reg};
use crate::interrupt::set_sie_mask;
use crate::structures::*;
use log::debug;
use riscv::register::{stvec, utvec::TrapMode};
use sel4_common::arch::config::RESET_CYCLES;
use sel4_common::sel4_config::*;
use sel4_common::{
    arch::{config::KERNEL_ELF_BASE, get_time, set_timer},
    BIT,
};
use sel4_vspace::activate_kernel_vspace;
use sel4_vspace::*;

pub fn init_cpu() {
    activate_kernel_vspace();
    extern "C" {
        fn trap_entry();
    }
    unsafe {
        stvec::write(trap_entry as usize, TrapMode::Direct);
    }
    #[cfg(feature = "enable_smp")]
    {
        set_sie_mask(BIT!(SIE_SEIE) | BIT!(SIE_STIE) | BIT!(SIE_SSIE));
    }
    #[cfg(not(feature = "enable_smp"))]
    {
        set_sie_mask(BIT!(SIE_SEIE) | BIT!(SIE_STIE));
    }
    set_timer(get_time() + RESET_CYCLES);

    unsafe {
        set_fs_off();
    }
    #[cfg(feature = "have_fpu")]
    init_fpu();
}

pub fn init_freemem(ui_reg: region_t, dtb_p_reg: p_region_t) -> bool {
    extern "C" {
        fn ki_end();
    }
    unsafe {
        res_reg[0].start = paddr_to_pptr(kpptr_to_paddr(KERNEL_ELF_BASE));
        res_reg[0].end = paddr_to_pptr(kpptr_to_paddr(ki_end as usize));
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
    if index >= NUM_RESERVED_REGIONS {
        debug!("ERROR: no slot to add user image to reserved regions\n");
        return false;
    }
    unsafe {
        res_reg[index] = ui_reg;
        index += 1;
        rust_init_freemem(avail_p_regs_size, avail_p_regs_addr, index, res_reg.clone())
    }
}
