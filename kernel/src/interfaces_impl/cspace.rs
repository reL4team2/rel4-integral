use core::usize;

// use crate::ffi::tcbDebugRemove;
use crate::arch::fpu::fpu_thread_delete;
use crate::interrupt::{deleting_irq_handler, is_irq_pending, set_irq_state_by_index, IRQState};
use crate::kernel::boot::current_lookup_fault;
use crate::syscall::safe_unbind_notification;
use sel4_common::sel4_config::{
    CONFIG_MAX_NUM_WORK_UNITS_PER_PREEMPTION, TCB_CNODE_ENTRIES, TCB_CTABLE, TCB_VTABLE,
};
use sel4_common::structures::exception_t;
#[cfg(feature = "kernel_mcs")]
use sel4_common::structures_gen::call_stack;
use sel4_common::structures_gen::{cap, cap_null_cap, cap_tag, endpoint, notification};
use sel4_common::utils::convert_to_mut_type_ref;
#[cfg(feature = "kernel_mcs")]
use sel4_common::utils::convert_to_option_mut_type_ref;
use sel4_cspace::capability::cap_func;
use sel4_cspace::compatibility::{zombie_new, ZOMBIE_TYPE_ZOMBIE_TCB};
use sel4_cspace::interface::FinaliseCapRet;
use sel4_ipc::{endpoint_func, notification_func, Transfer};
use sel4_task::{get_currenct_thread, ksWorkUnitsCompleted, tcb_t};
#[cfg(feature = "kernel_mcs")]
use sel4_task::{
    is_cur_domain_expired, get_consumed, get_current_sc, reply::reply_t, sched_context::sched_context_t,
    update_timestamp, ThreadState,
};
#[cfg(target_arch = "riscv64")]
use sel4_vspace::find_vspace_for_asid;
#[cfg(target_arch = "aarch64")]
use sel4_vspace::unmap_page_table;
use sel4_vspace::{asid_pool_t, asid_t, delete_asid, delete_asid_pool, unmap_page, PTE};

#[cfg(target_arch = "riscv64")]
#[no_mangle]
pub fn arch_finalise_cap(capability: &cap, final_: bool) -> FinaliseCapRet {
    let mut fc_ret = FinaliseCapRet {
        remainder: cap_null_cap::new().unsplay(),
        cleanupInfo: cap_null_cap::new().unsplay(),
    };
    match capability.get_tag() {
        cap_tag::cap_frame_cap => {
            if cap::cap_frame_cap(capability).get_capFMappedASID() != 0 {
                match unmap_page(
                    cap::cap_frame_cap(capability).get_capFSize() as usize,
                    cap::cap_frame_cap(capability).get_capFMappedASID() as usize,
                    cap::cap_frame_cap(capability).get_capFMappedAddress() as usize,
                    cap::cap_frame_cap(capability).get_capFBasePtr() as usize,
                ) {
                    Err(lookup_fault) => unsafe { current_lookup_fault = lookup_fault },
                    _ => {}
                }
            }
        }

        cap_tag::cap_page_table_cap => {
            if final_ && cap::cap_page_table_cap(capability).get_capPTIsMapped() != 0 {
                let asid = cap::cap_page_table_cap(capability).get_capPTMappedASID() as usize;
                let find_ret = find_vspace_for_asid(asid);
                let pte = cap::cap_page_table_cap(capability).get_capPTBasePtr() as usize;
                if find_ret.status == exception_t::EXCEPTION_NONE
                    && find_ret.vspace_root.unwrap() as usize == pte
                {
                    deleteASID(asid, pte as *mut PTE);
                } else {
                    convert_to_mut_type_ref::<PTE>(pte).unmap_page_table(
                        asid,
                        cap::cap_page_table_cap(capability).get_capPTMappedAddress() as usize,
                    );
                }
                if let Some(lookup_fault) = find_ret.lookup_fault {
                    unsafe {
                        current_lookup_fault = lookup_fault;
                    }
                }
            }
        }

        cap_tag::cap_asid_pool_cap => {
            if final_ {
                deleteASIDPool(
                    cap::cap_asid_pool_cap(capability).get_capASIDBase() as usize,
                    cap::cap_asid_pool_cap(capability).get_capASIDPool() as *mut asid_pool_t,
                );
            }
        }
        _ => {}
    }
    fc_ret.remainder = cap_null_cap::new().unsplay();
    fc_ret.cleanupInfo = cap_null_cap::new().unsplay();
    fc_ret
}

#[cfg(target_arch = "aarch64")]
pub fn arch_finalise_cap(capability: &cap, final_: bool) -> FinaliseCapRet {
    use sel4_common::utils::ptr_to_mut;

    let mut fc_ret = FinaliseCapRet {
        remainder: cap_null_cap::new().unsplay(),
        cleanupInfo: cap_null_cap::new().unsplay(),
    };
    match capability.get_tag() {
        cap_tag::cap_frame_cap => {
            if cap::cap_frame_cap(capability).get_capFMappedASID() != 0 {
                match unmap_page(
                    cap::cap_frame_cap(capability).get_capFSize() as usize,
                    cap::cap_frame_cap(capability).get_capFMappedASID() as usize,
                    cap::cap_frame_cap(capability).get_capFMappedAddress() as usize,
                    cap::cap_frame_cap(capability).get_capFBasePtr() as usize,
                ) {
                    Err(fault) => unsafe { current_lookup_fault = fault },
                    _ => {}
                }
            }
        }
        cap_tag::cap_vspace_cap => {
            if final_ && cap::cap_vspace_cap(capability).get_capVSIsMapped() == 1 {
                deleteASID(
                    cap::cap_vspace_cap(capability).get_capVSMappedASID() as usize,
                    cap::cap_vspace_cap(capability).get_capVSBasePtr() as _,
                );
            }
        }
        // cap_tag::CapPageGlobalDirectoryCap => {
        //     if final_ && cap.get_pgd_is_mapped() == 1 {
        //         deleteASID(cap.get_pgd_is_mapped(), cap.get_pgd_base_ptr() as _);
        //     }
        // }
        // cap_tag::CapPageUpperDirectoryCap => {
        //     if final_ && cap.get_pud_is_mapped() == 1 {
        //         let pud = ptr_to_mut(cap.get_pt_base_ptr() as *mut PUDE);
        //         unmap_page_upper_directory(
        //             cap.get_pud_mapped_asid(),
        //             cap.get_pud_mapped_address(),
        //             pud,
        //         );
        //     }
        // }
        // cap_tag::CapPageDirectoryCap => {
        //     if final_ && cap.get_pd_is_mapped() == 1 {
        //         let pd = ptr_to_mut(cap.get_pt_base_ptr() as *mut PDE);
        //         unmap_page_directory(cap.get_pd_mapped_asid(), cap.get_pd_mapped_address(), pd);
        //     }
        // }
        cap_tag::cap_page_table_cap => {
            if final_ && cap::cap_page_table_cap(capability).get_capPTIsMapped() == 1 {
                let pte =
                    ptr_to_mut(cap::cap_page_table_cap(capability).get_capPTBasePtr() as *mut PTE);
                unmap_page_table(
                    cap::cap_page_table_cap(capability).get_capPTMappedASID() as usize,
                    cap::cap_page_table_cap(capability).get_capPTMappedAddress() as usize,
                    pte,
                );
            }
        }
        cap_tag::cap_asid_pool_cap => {
            if final_ {
                deleteASIDPool(
                    cap::cap_asid_pool_cap(capability).get_capASIDBase() as usize,
                    cap::cap_asid_pool_cap(capability).get_capASIDPool() as *mut asid_pool_t,
                );
            }
        }
        cap_tag::cap_asid_control_cap => {}
        _ => {}
    }
    fc_ret.remainder = cap_null_cap::new().unsplay();
    fc_ret.cleanupInfo = cap_null_cap::new().unsplay();
    fc_ret
}

#[no_mangle]
pub fn finalise_cap(capability: &cap, _final: bool, _exposed: bool) -> FinaliseCapRet {
    let mut fc_ret = FinaliseCapRet {
        remainder: cap_null_cap::new().unsplay(),
        cleanupInfo: cap_null_cap::new().unsplay(),
    };

    if capability.is_arch_cap() {
        // For Removing Warnings
        // #[cfg(target_arch = "aarch64")]
        // unsafe {
        //     return arch_finalise_cap(cap, _final);
        // }
        // #[cfg(target_arch = "riscv64")]
        return arch_finalise_cap(capability, _final);
    }
    match capability.get_tag() {
        cap_tag::cap_endpoint_cap => {
            if _final {
                // cancelAllIPC(cap.get_ep_ptr() as *mut endpoint_t);
                convert_to_mut_type_ref::<endpoint>(
                    cap::cap_endpoint_cap(capability).get_capEPPtr() as usize,
                )
                .cancel_all_ipc()
            }
            fc_ret.remainder = cap_null_cap::new().unsplay();
            fc_ret.cleanupInfo = cap_null_cap::new().unsplay();
            return fc_ret;
        }
        cap_tag::cap_notification_cap => {
            if _final {
                let ntfn = convert_to_mut_type_ref::<notification>(
                    cap::cap_notification_cap(capability).get_capNtfnPtr() as usize,
                );
                #[cfg(feature = "kernel_mcs")]
                if let Some(sc) = convert_to_option_mut_type_ref::<sched_context_t>(
                    ntfn.get_ntfnSchedContext() as usize,
                ) {
                    sc.sched_context_unbind_ntfn();
                }
                ntfn.safe_unbind_tcb();
                ntfn.cacncel_all_signal();
            }
            fc_ret.remainder = cap_null_cap::new().unsplay();
            fc_ret.cleanupInfo = cap_null_cap::new().unsplay();
            return fc_ret;
        }
        cap_tag::cap_reply_cap => {
            #[cfg(feature = "kernel_mcs")]
            if _final {
                if let Some(reply) = convert_to_option_mut_type_ref::<reply_t>(
                    cap::cap_reply_cap(capability).get_capReplyPtr() as usize,
                ) {
                    if reply.replyTCB != 0 {
                        match convert_to_mut_type_ref::<tcb_t>(reply.replyTCB).get_state() {
                            ThreadState::ThreadStateBlockedOnReply => {
                                reply.remove(convert_to_mut_type_ref::<tcb_t>(reply.replyTCB));
                            }
                            ThreadState::ThreadStateBlockedOnReceive => {
                                convert_to_mut_type_ref::<tcb_t>(reply.replyTCB).cancel_ipc();
                            }
                            _ => {
                                panic!("invalid tcb state");
                            }
                        }
                    }
                }
            }
            fc_ret.remainder = cap_null_cap::new().unsplay();
            fc_ret.cleanupInfo = cap_null_cap::new().unsplay();
            return fc_ret;
        }
        cap_tag::cap_null_cap | cap_tag::cap_domain_cap => {
            fc_ret.remainder = cap_null_cap::new().unsplay();
            fc_ret.cleanupInfo = cap_null_cap::new().unsplay();
            return fc_ret;
        }
        _ => {
            if _exposed {
                panic!("finalise_cap: failed to finalise immediately.");
            }
        }
    }

    match capability.get_tag() {
        cap_tag::cap_cnode_cap => {
            return if _final {
                fc_ret.remainder = zombie_new(
                    1usize << cap::cap_cnode_cap(capability).get_capCNodeRadix() as usize,
                    cap::cap_cnode_cap(capability).get_capCNodeRadix() as usize,
                    cap::cap_cnode_cap(capability).get_capCNodePtr() as usize,
                );
                fc_ret.cleanupInfo = cap_null_cap::new().unsplay();
                fc_ret
            } else {
                fc_ret.remainder = cap_null_cap::new().unsplay();
                fc_ret.cleanupInfo = cap_null_cap::new().unsplay();
                fc_ret
            }
        }
        cap_tag::cap_thread_cap => {
            if _final {
                let tcb = convert_to_mut_type_ref::<tcb_t>(
                    cap::cap_thread_cap(capability).get_capTCBPtr() as usize,
                );
                #[cfg(feature = "enable_smp")]
                crate::smp::ipi::remote_tcb_stall(tcb);
                let cte_ptr = tcb.get_cspace_mut_ref(TCB_CTABLE);
                safe_unbind_notification(tcb);
                #[cfg(feature = "kernel_mcs")]
                if let Some(sc) =
                    convert_to_option_mut_type_ref::<sched_context_t>(tcb.tcbSchedContext)
                {
                    sc.sched_context_unbind_tcb(tcb);
                    if sc.scYieldFrom != 0 {
                        convert_to_mut_type_ref::<tcb_t>(sc.scYieldFrom)
                            .schedContext_completeYieldTo();
                    }
                }
                tcb.cancel_ipc();
                tcb.suspend();
                #[cfg(feature = "have_fpu")]
                fpu_thread_delete(tcb);
                // #[cfg(feature="DEBUG_BUILD")]
                // unsafe {
                //     tcbDebugRemove(tcb as *mut tcb_t);
                // }
                fc_ret.remainder =
                    zombie_new(TCB_CNODE_ENTRIES, ZOMBIE_TYPE_ZOMBIE_TCB, cte_ptr.get_ptr());
                fc_ret.cleanupInfo = cap_null_cap::new().unsplay();
                return fc_ret;
            }
        }
        #[cfg(feature = "kernel_mcs")]
        cap_tag::cap_sched_context_cap => {
            if _final {
                let sc = convert_to_mut_type_ref::<sched_context_t>(
                    cap::cap_sched_context_cap(capability).get_capSCPtr() as usize,
                );
                sc.sched_context_unbind_all_tcbs();
                sc.sched_context_unbind_ntfn();
                if sc.scReply != 0 {
                    assert!(
                        convert_to_mut_type_ref::<reply_t>(sc.scReply)
                            .replyNext
                            .get_isHead()
                            != 0
                    );
                    convert_to_mut_type_ref::<reply_t>(sc.scReply).replyNext =
                        call_stack::new(0, 0);
                    sc.scReply = 0;
                }
                if sc.scYieldFrom != 0 {
                    convert_to_mut_type_ref::<tcb_t>(sc.scYieldFrom).schedContext_completeYieldTo();
                }
                sc.scRefillMax = 0;
                fc_ret.remainder = cap_null_cap::new().unsplay();
                fc_ret.cleanupInfo = cap_null_cap::new().unsplay();
                return fc_ret;
            }
        }
        cap_tag::cap_zombie_cap => {
            fc_ret.remainder = capability.clone();
            fc_ret.cleanupInfo = cap_null_cap::new().unsplay();
            return fc_ret;
        }
        cap_tag::cap_irq_handler_cap => {
            if _final {
                let irq = cap::cap_irq_handler_cap(capability).get_capIRQ() as usize;
                deleting_irq_handler(irq);
                fc_ret.remainder = cap_null_cap::new().unsplay();
                fc_ret.cleanupInfo = capability.clone();
                return fc_ret;
            }
        }
        _ => {
            fc_ret.remainder = cap_null_cap::new().unsplay();
            fc_ret.cleanupInfo = cap_null_cap::new().unsplay();
            return fc_ret;
        }
    }
    fc_ret.remainder = cap_null_cap::new().unsplay();
    fc_ret.cleanupInfo = cap_null_cap::new().unsplay();
    return fc_ret;
}

#[no_mangle]
pub fn post_cap_deletion(capability: &cap) {
    if capability.get_tag() == cap_tag::cap_irq_handler_cap {
        let irq = cap::cap_irq_handler_cap(capability).get_capIRQ() as usize;
        // deleted_irq_handler
        set_irq_state_by_index(IRQState::IRQInactive, irq);
    }
}

#[no_mangle]
pub fn preemption_point() -> exception_t {
    unsafe {
        ksWorkUnitsCompleted += 1;
        if ksWorkUnitsCompleted >= CONFIG_MAX_NUM_WORK_UNITS_PER_PREEMPTION {
            ksWorkUnitsCompleted = 0;

            #[cfg(feature = "kernel_mcs")]
            {
                update_timestamp();
                let sc = get_current_sc();
                if !(sc.sc_active() && sc.refill_sufficient(get_consumed()))
                    || is_cur_domain_expired()
                    || is_irq_pending()
                {
                    return exception_t::EXCEPTION_PREEMTED;
                }
            }
            #[cfg(not(feature = "kernel_mcs"))]
            if is_irq_pending() {
                return exception_t::EXCEPTION_PREEMTED;
            }
        }
        exception_t::EXCEPTION_NONE
    }
}

#[no_mangle]
#[cfg(target_arch = "riscv64")]
pub fn deleteASID(asid: asid_t, vspace: *mut PTE) {
    unsafe {
        if let Err(lookup_fault) = delete_asid(
            asid,
            vspace,
            &get_currenct_thread().get_cspace(TCB_VTABLE).capability,
        ) {
            current_lookup_fault = lookup_fault;
        }
    }
}

#[no_mangle]
#[cfg(target_arch = "aarch64")]
pub fn deleteASID(asid: asid_t, vspace: *mut PTE) {
    unsafe {
        if let Err(lookup_fault) = delete_asid(
            asid,
            vspace,
            &get_currenct_thread().get_cspace(TCB_VTABLE).capability,
        ) {
            current_lookup_fault = lookup_fault;
        }
    }
}

#[no_mangle]
#[cfg(target_arch = "aarch64")]
pub fn deleteASIDPool(asid_base: asid_t, pool: *mut asid_pool_t) {
    unsafe {
        if let Err(lookup_fault) = delete_asid_pool(
            asid_base,
            pool,
            &get_currenct_thread().get_cspace(TCB_VTABLE).capability,
        ) {
            current_lookup_fault = lookup_fault;
        }
    }
}

#[no_mangle]
#[cfg(target_arch = "riscv64")]
pub fn deleteASIDPool(asid_base: asid_t, pool: *mut asid_pool_t) {
    unsafe {
        if let Err(lookup_fault) = delete_asid_pool(
            asid_base,
            pool,
            &get_currenct_thread().get_cspace(TCB_VTABLE).capability,
        ) {
            current_lookup_fault = lookup_fault;
        }
    }
}
