use crate::arch::fpu::{init_fpu, set_fs_off};
use crate::boot::rust_init_freemem;
use crate::boot::{avail_p_regs_addr, avail_p_regs_size, res_reg};
use crate::interrupt::set_sie_mask;
use log::debug;
use rel4_arch::basic::{PRegion, Region};
use riscv::register::{stvec, utvec::TrapMode};
use sel4_common::arch::config::RESET_CYCLES;
use sel4_common::arch::{config::KERNEL_ELF_BASE, get_time, set_timer};
use sel4_common::sel4_config::*;
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
        set_sie_mask(bit!(SIE_SEIE) | bit!(SIE_STIE) | bit!(SIE_SSIE));
    }
    #[cfg(not(feature = "enable_smp"))]
    {
        set_sie_mask(bit!(SIE_SEIE) | bit!(SIE_STIE));
    }
    set_timer(get_time() + RESET_CYCLES);

    unsafe {
        set_fs_off();
    }
    #[cfg(feature = "have_fpu")]
    init_fpu();
}

pub fn init_freemem(ui_reg: Region, dtb_p_reg: PRegion) -> bool {
    extern "C" {
        fn ki_end();
    }
    unsafe {
        res_reg[0].start = kpptr_to_paddr(KERNEL_ELF_BASE).to_pptr();
        res_reg[0].end = kpptr_to_paddr(ki_end as usize).to_pptr();
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
