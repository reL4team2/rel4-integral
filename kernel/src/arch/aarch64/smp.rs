use crate::BIT;
use sel4_common::utils::cpu_id;
use crate::smp::{*, ipi::*, lock::*};

#[derive(Debug, Clone, Copy)]
pub enum ipi_remote_call {
    IpiRemoteCall_Stall = 0,
    IpiRemoteCall_InvalidateTranslationSingle,
    IpiRemoteCall_InvalidateTranslationASID,
    IpiRemoteCall_InvalidateTranslationAll,
    IpiRemoteCall_switchFpuOwner,
    IpiRemoteCall_MaskPrivateInterrupt,
    IpiNumArchRemoteCall,
}

pub fn handle_remote_call(call: ipi_remote_call, arg0: usize, arg1: usize, arg2: usize, irq_path: bool) {
    if crate::smp::clh_is_ipi_pending(cpu_id()) {
        match call {
            ipi_remote_call::IpiRemoteCall_Stall => { crate::smp::ipi::ipi_stall_core_cb(irq_path); }
            _ => { sel4_common::println!("handle_remote_call: call: {:?}, arg0: {}, arg1: {}, arg2: {}", call, arg0, arg1, arg2); }
        }
        crate::smp::clh_set_ipi(cpu_id(), 0);
        unsafe { crate::smp::ipi::ipi_wait() };
    }
}

#[inline]
pub fn arch_pause() {
    // TODO
}