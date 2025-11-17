pub mod interface;
pub mod mm;
mod root_server;
mod untyped;
mod utils;

use core::mem::size_of;

use log::debug;
use rel4_arch::basic::{PAddr, PRegion, Region};
#[cfg(feature = "kernel_mcs")]
use sel4_common::platform::{timer, Timer_func};
use sel4_common::{arch::config::PADDR_TOP, sel4_config::*};
use spin::Mutex;

use crate::structures::{ndks_boot_t, BootInfo, BootInfoHeader, SlotRegion};

#[cfg(target_arch = "aarch64")]
pub use mm::reserve_region;
pub use mm::{avail_p_regs_addr, avail_p_regs_size, res_reg, rust_init_freemem};
pub use root_server::rootserver;
use sel4_task::*;

pub use root_server::root_server_init;
pub use untyped::create_untypeds;

// #[link_section = ".boot.bss"]
pub static ksNumCPUs: Mutex<usize> = Mutex::new(0);
#[cfg(feature = "enable_smp")]
#[link_section = ".boot.bss"]
pub static node_boot_lock: Mutex<usize> = Mutex::new(0);

#[no_mangle]
#[link_section = ".boot.bss"]
pub static mut ndks_boot: ndks_boot_t = ndks_boot_t {
    reserved: [PRegion::empty(); MAX_NUM_RESV_REG],
    resv_count: 0,
    freemem: [Region::empty(); MAX_NUM_FREEMEM_REG],
    bi_frame: 0 as *mut BootInfo,
    slot_pos_cur: SEL4_NUM_INITIAL_CAPS,
};

pub fn calculate_extra_bi_size_bits(size: usize) -> usize {
    if size == 0 {
        return 0;
    }

    let clzl_ret = round_up!(size, SEL4_PAGE_BITS).leading_zeros() as usize;
    let mut msb = SEL4_WORD_BITS - 1 - clzl_ret;
    if size > bit!(msb) {
        msb += 1;
    }
    return msb;
}

pub fn init_dtb(
    dtb_size: usize,
    dtb_phys_addr: PAddr,
    extra_bi_size: &mut usize,
) -> Option<PRegion> {
    let mut dtb_p_reg = PRegion::empty();
    if dtb_size > 0 {
        let dtb_phys_end = dtb_phys_addr + dtb_size;
        if dtb_phys_end < dtb_phys_addr {
            debug!(
                "ERROR: DTB location at {}
             len {} invalid",
                dtb_phys_addr.raw(),
                dtb_size
            );
            return None;
        }
        if dtb_phys_end.raw() >= PADDR_TOP {
            debug!(
                "ERROR: DTB at [{}..{}] exceeds PADDR_TOP ({})\n",
                dtb_phys_addr.raw(),
                dtb_phys_end.raw(),
                PADDR_TOP
            );
            return None;
        }

        (*extra_bi_size) += size_of::<BootInfoHeader>() + dtb_size;
        dtb_p_reg = PRegion {
            start: dtb_phys_addr,
            end: dtb_phys_end,
        };
    }
    Some(dtb_p_reg)
}

pub fn init_bootinfo(dtb_size: usize, dtb_phys_addr: PAddr, extra_bi_size: usize) {
    let mut extra_bi_offset = 0;
    let mut header: BootInfoHeader = BootInfoHeader { id: 0, len: 0 };
    if dtb_size > 0 {
        header.id = SEL4_BOOTINFO_HEADER_FDT;
        header.len = size_of::<BootInfoHeader>() + dtb_size;
        unsafe {
            *((rootserver.extra_bi + extra_bi_offset) as *mut BootInfoHeader) = header.clone();
        }
        extra_bi_offset += size_of::<BootInfoHeader>();
        let src = unsafe {
            core::slice::from_raw_parts(dtb_phys_addr.to_pptr().get_ptr::<u8>(), dtb_size)
        };
        unsafe {
            let dst = core::slice::from_raw_parts_mut(
                (rootserver.extra_bi + extra_bi_offset) as *mut u8,
                dtb_size,
            );
            dst.copy_from_slice(src);
        }
        extra_bi_offset += dtb_size;
    }
    if extra_bi_size > extra_bi_offset {
        header.id = SEL4_BOOTINFO_HEADER_PADDING;
        header.len = extra_bi_size - extra_bi_offset;
        unsafe {
            *((rootserver.extra_bi + extra_bi_offset) as *mut BootInfoHeader) = header.clone();
        }
    }
}

pub fn bi_finalise(dtb_size: usize, dtb_phys_addr: PAddr, extra_bi_size: usize) {
    unsafe {
        (*ndks_boot.bi_frame).empty = SlotRegion {
            start: ndks_boot.slot_pos_cur,
            end: bit!(CONFIG_ROOT_CNODE_SIZE_BITS),
        };
    }
    init_bootinfo(dtb_size, dtb_phys_addr, extra_bi_size);
}

pub fn init_core_state(scheduler_action: *mut tcb_t) {
    // unsafe {
    // #[cfg(feature = "enable_smp")]
    // if scheduler_action as usize != 0 && scheduler_action as usize != 1 {
    //     tcbDebugAppend(scheduler_action);
    // }
    // let idle_thread = {
    //     #[cfg(not(feature = "enable_smp"))]
    //     {
    //         ksIdleThread as *mut tcb_t
    //     }
    //     #[cfg(feature = "enable_smp")]
    //     {
    //         ksSMP[cpu_id()].ksIdleThread as *mut tcb_t
    //     }
    // };
    // tcbDebugAppend(idle_thread);
    // }

    SET_NODE_STATE!(ksSchedulerAction = scheduler_action as usize);
    set_current_thread(get_idle_thread());
    // TODO: MCS
    // #ifdef CONFIG_KERNEL_MCS
    // 	NODE_STATE(ksCurSC) = NODE_STATE(ksCurThread->tcbSchedContext);
    // 	NODE_STATE(ksConsumed) = 0;
    // 	NODE_STATE(ksReprogram) = true;
    // 	NODE_STATE(ksReleaseHead) = NULL;
    // 	NODE_STATE(ksCurTime) = get_current_time();
    // #endif
    #[cfg(feature = "kernel_mcs")]
    {
        SET_NODE_STATE!(ksCurSC = get_currenct_thread().tcbSchedContext);
        SET_NODE_STATE!(ksConsumed = 0);
        SET_NODE_STATE!(ksReprogram = true);
        SET_NODE_STATE!(ksReleaseQueue = tcb_queue_t { head: 0, tail: 0 });
        SET_NODE_STATE!(ksCurTime = timer.get_current_time());
    }
}
