use log::debug;
use sel4_common::arch::config::KERNEL_ELF_BASE;
use sel4_common::println;
use sel4_common::{sel4_config::PAGE_BITS, BIT};
use sel4_task::{create_idle_thread, tcb_t, SCHEDULER_ACTION_RESUME_CURRENT_THREAD};
use sel4_vspace::{kpptr_to_paddr, rust_map_kernel_window};

use crate::arch::aarch64::platform::{clean_invalidate_l1_caches, init_cpu, invalidate_local_tlb};

use crate::{
    arch::init_freemem,
    boot::{
        bi_finalise, calculate_extra_bi_size_bits, create_untypeds, init_core_state, init_dtb,
        ksNumCPUs, ndks_boot, paddr_to_pptr_reg, root_server_init,
    },
    structures::{p_region_t, v_region_t, SlotRegion},
};

use sel4_common::sel4_config::{BI_FRAME_SIZE_BITS, USER_TOP};

use super::platform::init_irq_controller;
use crate::interrupt::{intStateIRQNodeToR, mask_interrupt, set_irq_state_by_irq, IrqState};

#[cfg(feature = "ENABLE_SMP")]
use core::arch::asm;

#[cfg(feature = "ENABLE_SMP")]
use crate::ffi::{clh_lock_acquire, clh_lock_init};

#[cfg(feature = "ENABLE_SMP")]
use sel4_common::utils::cpu_id;

#[cfg(feature = "ENABLE_SMP")]
use crate::boot::node_boot_lock;

pub fn try_init_kernel(
    ui_p_reg_start: usize,
    ui_p_reg_end: usize,
    pv_offset: isize,
    v_entry: usize,
    dtb_phys_addr: usize,
    dtb_size: usize,
    ki_boot_end: usize,
) -> bool {
    intStateIRQNodeToR();
    // Init logging for log crate
    sel4_common::logging::init();
    let boot_mem_reuse_p_reg = p_region_t {
        start: kpptr_to_paddr(KERNEL_ELF_BASE),
        end: kpptr_to_paddr(ki_boot_end as usize),
    };
    let boot_mem_reuse_reg = paddr_to_pptr_reg(&boot_mem_reuse_p_reg);
    let ui_p_reg = p_region_t {
        start: ui_p_reg_start,
        end: ui_p_reg_end,
    };
    let ui_reg = paddr_to_pptr_reg(&ui_p_reg);

    let mut extra_bi_size = 0;
    let ui_v_reg = v_region_t {
        start: (ui_p_reg_start as isize - pv_offset) as usize,
        end: (ui_p_reg_end as isize - pv_offset) as usize,
    };
    let ipcbuf_vptr = ui_v_reg.end;
    let bi_frame_vptr = ipcbuf_vptr + BIT!(PAGE_BITS);
    let extra_bi_frame_vptr = bi_frame_vptr + BIT!(BI_FRAME_SIZE_BITS);

    // Map kernel window area
    rust_map_kernel_window();

    // Initialize cpu
    let inited = init_cpu();
    // Initialize the drivers used by the kernel.
    driver_collect::init();
    log::debug!("init_cpu: {}", inited);

    // Initialize platform
    // sel4_common::ffi_call!(init_plat);
    init_plat();

    let dtb_p_reg = init_dtb(dtb_size, dtb_phys_addr, &mut extra_bi_size);
    if dtb_p_reg.is_none() {
        return false;
    }

    let extra_bi_size_bits = calculate_extra_bi_size_bits(extra_bi_size);

    let it_v_reg = v_region_t {
        start: ui_v_reg.start,
        end: extra_bi_frame_vptr + BIT!(extra_bi_size_bits),
    };

    if it_v_reg.end >= USER_TOP {
        debug!(
            "ERROR: userland image virt [{}..{}]
        exceeds USER_TOP ({})\n",
            it_v_reg.start, it_v_reg.end, USER_TOP
        );
        return false;
    }

    // FIXED: init_freemem should be p_region_t, but is region_t before.
    if !init_freemem(ui_p_reg.clone(), dtb_p_reg.unwrap().clone()) {
        debug!("ERROR: free memory management initialization failed\n");
        return false;
    }
    if let Some((initial_thread, root_cnode_cap)) = root_server_init(
        it_v_reg,
        extra_bi_size_bits,
        ipcbuf_vptr,
        bi_frame_vptr,
        extra_bi_size,
        extra_bi_frame_vptr,
        ui_reg,
        pv_offset,
        v_entry,
    ) {
        create_idle_thread();
        clean_invalidate_l1_caches();
        init_core_state(initial_thread);
        if !create_untypeds(&root_cnode_cap, boot_mem_reuse_reg) {
            debug!("ERROR: could not create untypteds for kernel image boot memory");
        }
        unsafe {
            (*ndks_boot.bi_frame).sharedFrames = SlotRegion { start: 0, end: 0 };

            bi_finalise(dtb_size, dtb_phys_addr, extra_bi_size);
        }
        clean_invalidate_l1_caches();
        invalidate_local_tlb();
        // debug!("release_secondary_cores start");
        *ksNumCPUs.lock() = 1;
        #[cfg(feature = "ENABLE_SMP")]
        {
            use crate::ffi::{clh_lock_acquire, clh_lock_init};
            use sel4_common::utils::cpu_id;
            unsafe {
                clh_lock_init();
                release_secondary_cpus();
                clh_lock_acquire(cpu_id(), false);
            }
        }

        println!("Booting all finished, dropped to user space");
        println!("\n");
    } else {
        return false;
    }

    true
}

#[cfg(feature = "ENABLE_SMP")]
#[inline(always)]
pub fn try_init_kernel_secondary_core(hartid: usize, core_id: usize) -> bool {
    use core::ops::AddAssign;
    use sel4_common::arch::config::{irq_remote_call_ipi, irq_reschedule_ipi};
    use sel4_common::platform::KERNEL_TIMER_IRQ;
    while node_boot_lock.lock().eq(&0) {}
    // Initialize cpu
    init_cpu();

    for i in 0..sel4_common::platform::NUM_PPI {
        mask_interrupt(true, i);
    }
    set_irq_state_by_irq(IrqState::IRQIPI, irq_remote_call_ipi);
    set_irq_state_by_irq(IrqState::IRQIPI, irq_reschedule_ipi);
    set_irq_state_by_irq(IrqState::IRQTimer, KERNEL_TIMER_IRQ);

    unsafe { clh_lock_acquire(cpu_id(), false) };
    ksNumCPUs.lock().add_assign(1);
    init_core_state(SCHEDULER_ACTION_RESUME_CURRENT_THREAD as *mut tcb_t);

    true
}

#[cfg(feature = "ENABLE_SMP")]
pub(crate) fn release_secondary_cpus() {
    use sel4_common::sel4_config::CONFIG_MAX_NUM_NODES;
    *node_boot_lock.lock() = 1;
    while ksNumCPUs.lock().ne(&CONFIG_MAX_NUM_NODES) {}
}

fn init_plat() {
    init_irq_controller()
}
