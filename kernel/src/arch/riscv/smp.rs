use sel4_common::sel4_config::CONFIG_MAX_NUM_NODES;
use core::sync::atomic::{fence, Ordering};
use crate::BIT;
use sel4_common::arch::{hart_id_to_core_id, sbi_send_ipi};
use sel4_common::platform::IRQ_INVALID;
use sel4_common::arch::config::{IRQ_REMOTE_CALL_IPI, IRQ_RESCHEDULE_IPI};
use sel4_common::utils::cpu_id;
use crate::smp::clh_is_ipi_pending;

#[derive(Debug, Clone, Copy)]
pub enum ipi_remote_call {
    IpiRemoteCall_Stall = 0,
    IpiRemoteCall_InvalidateTranslationSingle,
    IpiNumArchRemoteCall,
}

static mut ipi_irq: [usize; CONFIG_MAX_NUM_NODES] = [IRQ_INVALID; CONFIG_MAX_NUM_NODES];

#[inline(always)]
pub fn arch_pause() {
    fence(Ordering::SeqCst);
}

pub fn ipi_send_target(irq: usize, target: usize) {
    let mask = BIT!(target);
    let core_id = hart_id_to_core_id(target);
    assert!(core_id < CONFIG_MAX_NUM_NODES);
    unsafe {
        assert!(ipi_irq[core_id] == IRQ_INVALID || ipi_irq[core_id] == IRQ_RESCHEDULE_IPI 
                || (ipi_irq[core_id] == IRQ_REMOTE_CALL_IPI && !clh_is_ipi_pending(core_id)));
        ipi_irq[core_id] = irq;
    }
    fence(Ordering::SeqCst);
    sbi_send_ipi(mask);
}

pub fn handle_remote_call(call: ipi_remote_call, arg0: usize, arg1: usize, arg2: usize, irq_path: bool) {
    if clh_is_ipi_pending(cpu_id()) {
        match call {
            ipi_remote_call::IpiRemoteCall_Stall => { crate::smp::ipi::ipi_stall_core_cb(irq_path); }
            _ => { sel4_common::println!("handle_remote_call: call: {:?}, arg0: {}, arg1: {}, arg2: {}", call, arg0, arg1, arg2); }
        }
        crate::smp::clh_set_ipi(cpu_id(), 0);
        unsafe {
            ipi_irq[cpu_id()] = IRQ_INVALID;
            crate::smp::ipi::ipi_wait() 
        };
    }
}

pub fn ipi_clear_irq(irq: usize) {
    unsafe {
        ipi_irq[cpu_id()] = IRQ_INVALID;
    }
}

pub fn ipi_get_irq() -> usize {
    unsafe {
        assert!(!(ipi_irq[cpu_id()] == IRQ_INVALID && clh_is_ipi_pending(cpu_id())));
        return ipi_irq[cpu_id()];
    }
}