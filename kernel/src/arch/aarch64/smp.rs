use crate::smp::ipi::do_remote_mask_op;
use sel4_common::sel4_config::CONFIG_MAX_NUM_NODES;
use sel4_common::utils::cpu_id;
use sel4_vspace::arch::{invalidate_local_tlb_asid, invalidate_local_tlb_va_asid};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum ipi_remote_call {
    IpiRemoteCall_Stall = 0,
    // in invalidateTLBByASIDVA
    IpiRemoteCall_InvalidateTranslationSingle,
    //findFreeHWASID invalidateTLBByASID
    IpiRemoteCall_InvalidateTranslationASID,
    // not used
    IpiRemoteCall_InvalidateTranslationAll,
    IpiRemoteCall_switchFpuOwner,
    // invokeIRQHandler_AckIRQ
    IpiRemoteCall_MaskPrivateInterrupt,
    IpiNumArchRemoteCall,
}

pub fn handle_remote_call(
    call: ipi_remote_call,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    irq_path: bool,
) {
    if crate::smp::clh_is_ipi_pending(cpu_id()) {
        match call {
            ipi_remote_call::IpiRemoteCall_Stall => {
                crate::smp::ipi::ipi_stall_core_cb(irq_path);
            }
            ipi_remote_call::IpiRemoteCall_switchFpuOwner => unsafe {
                crate::arch::fpu::switch_local_fpu_owner(arg0);
            },
            ipi_remote_call::IpiRemoteCall_InvalidateTranslationSingle => {
                invalidate_local_tlb_va_asid(arg0)
            }
            ipi_remote_call::IpiRemoteCall_InvalidateTranslationASID => {
                invalidate_local_tlb_asid(arg0)
            }
            ipi_remote_call::IpiRemoteCall_MaskPrivateInterrupt => {
                crate::interrupt::mask_interrupt(arg0 != 0, arg1)
            }
            _ => {
                log::warn!(
                    "handle_remote_call: call: {:?}, arg0: {}, arg1: {}, arg2: {}",
                    call,
                    arg0,
                    arg1,
                    arg2
                );
            }
        }
        crate::smp::clh_set_ipi(cpu_id(), 0);
        crate::smp::ipi::ipi_wait();
    }
}

#[inline]
pub fn arch_pause() {
    // TODO
}

/// doRemoteInvalidateTranslationASID in seL4
#[no_mangle]
pub fn remote_invalidate_tlb_asid(asid: sel4_vspace::asid_t) {
    let mask = crate::BIT!(CONFIG_MAX_NUM_NODES) - 1;
    do_remote_mask_op(
        ipi_remote_call::IpiRemoteCall_InvalidateTranslationASID,
        asid as usize,
        0,
        0,
        mask,
    );
}

/// doRemoteInvalidateTranslationSingle in seL4
#[no_mangle]
pub fn remote_invalidate_translation_single(vptr: usize) {
    let mask = crate::BIT!(CONFIG_MAX_NUM_NODES) - 1;
    do_remote_mask_op(
        ipi_remote_call::IpiRemoteCall_InvalidateTranslationSingle,
        vptr,
        0,
        0,
        mask,
    );
}

/// 和 seL4 有所不同，irq 就是 irq，不是 Index，方便 mask_interrupt
pub fn remote_mask_private_interrupt(cpu: usize, disable: bool, irq: usize) {
    do_remote_mask_op(
        ipi_remote_call::IpiRemoteCall_MaskPrivateInterrupt,
        disable as usize,
        irq,
        0,
        cpu,
    );
}
