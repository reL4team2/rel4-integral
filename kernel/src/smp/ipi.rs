use core::sync::atomic::{AtomicUsize, Ordering, fence};
use sel4_common::structures::{irq_t, irq_to_idx, idx_to_irq, irqt_to_irq};
use sel4_common::arch::config::{IRQ_REMOTE_CALL_IPI, IRQ_RESCHEDULE_IPI};
use sel4_common::utils::cpu_id;
use sel4_common::sel4_config::{CONFIG_MAX_NUM_NODES, WORD_BITS};
use sel4_task::{ThreadState, SCHEDULER_ACTION_RESUME_CURRENT_THREAD, tcb_t};
use crate::arch::{ipi_remote_call, cpu_index_to_id};
use crate::arch::arm_gic::ipi_send_targets;

pub const MAX_IPI_ARGS: usize = 3;

#[repr(align(64))]
struct ipi_sync_barrier {
    count: AtomicUsize,
    globalsense: usize,
}

impl ipi_sync_barrier {
    const fn new() -> Self {
        Self {
            count: AtomicUsize::new(0),
            globalsense: 0,
        }
    }
}

static mut ipi_args: [usize; MAX_IPI_ARGS] = [0; MAX_IPI_ARGS];
static mut barrier: ipi_sync_barrier = ipi_sync_barrier::new();
static mut total_core_barrier: usize = 0;
static mut remote_call: ipi_remote_call = ipi_remote_call::IpiRemoteCall_Stall;

#[inline]
fn get_ipi_arg(n: usize) -> usize {
    assert!(n < MAX_IPI_ARGS);
    let arg = unsafe {
        ipi_args[n]
    };
    return arg;
}

#[inline]
pub fn ipi_wait() {
    unsafe {
        let localsense = barrier.globalsense;
        let old = barrier.count.fetch_add(1, Ordering::AcqRel);
        if old == total_core_barrier {
            barrier.count.store(0, Ordering::Release);
            barrier.globalsense = !localsense;
        }

        while localsense == barrier.globalsense {
            crate::arch::arch_pause();
        }
    }
}

pub fn ipi_stall_core_cb(irq_path: bool) {
    let thread = sel4_task::get_currenct_thread();
    if super::clh_is_self_in_queue() && !irq_path {
        if thread.tcbState.get_tsType() == ThreadState::ThreadStateRunning as u64 {
            sel4_task::set_thread_state(thread, ThreadState::ThreadStateRestart);
        }
        thread.sched_enqueue();
        crate::boot::interface::switch_to_idle_thread();
        // TODO: mcs support
        sel4_task::set_ks_scheduler_action(SCHEDULER_ACTION_RESUME_CURRENT_THREAD);
        super::clh_set_ipi(cpu_id(), 0);

        // TODO: riscv clear irq
        unsafe { ipi_wait() };

        while super::clh_next_node_state(cpu_id()) != super::lock::clh_qnode_state::CLHState_Granted {
            crate::arch::arch_pause();
        }

        fence(Ordering::SeqCst);

        sel4_task::activateThread();
        crate::arch::restore_user_context();
    } else {
        thread.sched_enqueue();
        sel4_task::activateThread();
        sel4_task::set_ks_scheduler_action(SCHEDULER_ACTION_RESUME_CURRENT_THREAD);
    }
}

pub fn handle_ipi(irq: usize, irq_path: bool) {
    match irq {
        IRQ_REMOTE_CALL_IPI => {
            unsafe {crate::arch::handle_remote_call(remote_call, get_ipi_arg(0), get_ipi_arg(1), get_ipi_arg(2), irq_path);}
        }
        IRQ_RESCHEDULE_IPI => {
            sel4_task::reschedule_required();
            // TODO: riscv ifence_local
        }
        _ => sel4_common::println!("handle_ipi: unknown ipi: {}", irq),
    }
}

pub fn ipi_send_mask(irq: usize, mask: usize, block: bool) {
    let mut nr_target_cores: usize = 0;
    let mut target_cores: [usize; CONFIG_MAX_NUM_NODES] = [0; CONFIG_MAX_NUM_NODES];
    let mut mask2 = mask;
    while mask2 > 0 {
        let index = WORD_BITS - 1 - mask2.leading_zeros() as usize;
        if block {
            super::clh_set_ipi(index, 1);
            target_cores[nr_target_cores] = index;
            nr_target_cores += 1;
        } else {
            ipi_send_targets(irq, cpu_index_to_id(index));
        }
        mask2 &= !(crate::BIT!(index));
    }

    if nr_target_cores > 0 {
        fence(Ordering::SeqCst);
        for i in 0..nr_target_cores {
            ipi_send_targets(irq, cpu_index_to_id(target_cores[i]));
        }
    }
}

#[no_mangle]
pub fn do_mask_reschedule(mask: usize) {
    let mut mask2 = mask;
    mask2 &= !(crate::BIT!(cpu_id()));
    if mask2 != 0 {
        ipi_send_mask(IRQ_RESCHEDULE_IPI, mask2, false);
    }
}

pub fn do_remote_mask_op(func: ipi_remote_call, arg0: usize, arg1: usize, arg2: usize, mask: usize) {
    let mut mask2 = mask;
    mask2 &= !(crate::BIT!(cpu_id()));
    if mask2 != 0 {
        unsafe {
            ipi_args[0] = arg0;
            ipi_args[1] = arg1;
            ipi_args[2] = arg2;
            remote_call = func;
            total_core_barrier = mask.count_ones() as usize;
        }

        fence(Ordering::SeqCst);
        ipi_send_mask(IRQ_REMOTE_CALL_IPI, mask2, true);
        ipi_wait();
    }
}

pub fn do_remote_op(func: ipi_remote_call, arg0: usize, arg1: usize, arg2: usize, cpu: usize) {
    do_remote_mask_op(func, arg0, arg1, arg2, crate::BIT!(cpu));
}

pub fn do_remote_stall(cpu: usize) {
    do_remote_op(ipi_remote_call::IpiRemoteCall_Stall, 0, 0, 0, cpu);
}

pub fn remote_tcb_stall(tcb: &tcb_t) {
    // TODO: mcs support
    if tcb.tcbAffinity != cpu_id() && tcb.is_current() {
        do_remote_stall(tcb.tcbAffinity);
        tcb.update_ipi_reschedule_pending();
    }
}