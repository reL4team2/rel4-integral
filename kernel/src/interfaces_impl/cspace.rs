use core::usize;

use crate::config::CONFIG_MAX_NUM_WORK_UNITS_PER_PREEMPTION;
// use crate::ffi::tcbDebugRemove;
use crate::interrupt::{deletingIRQHandler, isIRQPending, setIRQState, IRQState};
use crate::kernel::boot::current_lookup_fault;
use crate::syscall::safe_unbind_notification;
use sel4_common::sel4_config::{tcbCNodeEntries, tcbCTable, tcbVTable};
use sel4_common::structures::exception_t;
#[cfg(feature = "KERNEL_MCS")]
use sel4_common::structures_gen::call_stack;
use sel4_common::structures_gen::{cap, cap_null_cap, cap_tag, endpoint, notification};
use sel4_common::utils::convert_to_mut_type_ref;
#[cfg(feature = "KERNEL_MCS")]
use sel4_common::utils::convert_to_option_mut_type_ref;
use sel4_cspace::capability::cap_func;
use sel4_cspace::compatibility::{ZombieType_ZombieTCB, Zombie_new};
use sel4_cspace::interface::finaliseCap_ret;
use sel4_ipc::{endpoint_func, notification_func, Transfer};
use sel4_task::{get_currenct_thread, ksWorkUnitsCompleted, tcb_t};
#[cfg(feature = "KERNEL_MCS")]
use sel4_task::{
    isCurDomainExpired, ksConsumed, ksCurSC, reply::reply_t, sched_context::sched_context_t,
    updateTimestamp, ThreadState,
};
#[cfg(target_arch="aarch64")]
use crate::arch::fpu::fpuThreadDelete;
#[cfg(target_arch = "riscv64")]
use sel4_vspace::find_vspace_for_asid;
#[cfg(target_arch = "aarch64")]
use sel4_vspace::unmap_page_table;
use sel4_vspace::{asid_pool_t, asid_t, delete_asid, delete_asid_pool, unmapPage, PTE};

#[cfg(target_arch = "riscv64")]
#[no_mangle]
pub fn Arch_finaliseCap(capability: &cap, final_: bool) -> finaliseCap_ret {
    let mut fc_ret = finaliseCap_ret {
        remainder: cap_null_cap::new().unsplay(),
        cleanupInfo: cap_null_cap::new().unsplay(),
    };
    match capability.get_tag() {
        cap_tag::cap_frame_cap => {
            if cap::cap_frame_cap(capability).get_capFMappedASID() != 0 {
                match unmapPage(
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
pub fn Arch_finaliseCap(capability: &cap, final_: bool) -> finaliseCap_ret {
    use sel4_common::utils::ptr_to_mut;

    let mut fc_ret = finaliseCap_ret {
        remainder: cap_null_cap::new().unsplay(),
        cleanupInfo: cap_null_cap::new().unsplay(),
    };
    match capability.get_tag() {
        cap_tag::cap_frame_cap => {
            if cap::cap_frame_cap(capability).get_capFMappedASID() != 0 {
                match unmapPage(
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
pub fn finaliseCap(capability: &cap, _final: bool, _exposed: bool) -> finaliseCap_ret {
    let mut fc_ret = finaliseCap_ret {
        remainder: cap_null_cap::new().unsplay(),
        cleanupInfo: cap_null_cap::new().unsplay(),
    };

    if capability.isArchCap() {
        // For Removing Warnings
        // #[cfg(target_arch = "aarch64")]
        // unsafe {
        //     return Arch_finaliseCap(cap, _final);
        // }
        // #[cfg(target_arch = "riscv64")]
        return Arch_finaliseCap(capability, _final);
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
                #[cfg(feature = "KERNEL_MCS")]
                if let Some(sc) = convert_to_option_mut_type_ref::<sched_context_t>(
                    ntfn.get_ntfnSchedContext() as usize,
                ) {
                    sc.schedContext_unbindNtfn();
                }
                ntfn.safe_unbind_tcb();
                ntfn.cacncel_all_signal();
            }
            fc_ret.remainder = cap_null_cap::new().unsplay();
            fc_ret.cleanupInfo = cap_null_cap::new().unsplay();
            return fc_ret;
        }
        cap_tag::cap_reply_cap => {
            #[cfg(feature = "KERNEL_MCS")]
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
                panic!("finaliseCap: failed to finalise immediately.");
            }
        }
    }

    match capability.get_tag() {
        cap_tag::cap_cnode_cap => {
            return if _final {
                fc_ret.remainder = Zombie_new(
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
                #[cfg(feature = "ENABLE_SMP")]
                unsafe {
                    crate::ffi::remoteTCBStall(tcb)
                };
                let cte_ptr = tcb.get_cspace_mut_ref(tcbCTable);
                safe_unbind_notification(tcb);
                #[cfg(feature = "KERNEL_MCS")]
                if let Some(sc) =
                    convert_to_option_mut_type_ref::<sched_context_t>(tcb.tcbSchedContext)
                {
                    sc.schedContext_unbindTCB(tcb);
                    if sc.scYieldFrom != 0 {
                        convert_to_mut_type_ref::<tcb_t>(sc.scYieldFrom)
                            .schedContext_completeYieldTo();
                    }
                }
                tcb.cancel_ipc();
                tcb.suspend();
				#[cfg(target_arch = "aarch64")]
				fpuThreadDelete(tcb);
                // #[cfg(feature="DEBUG_BUILD")]
                // unsafe {
                //     tcbDebugRemove(tcb as *mut tcb_t);
                // }
                fc_ret.remainder =
                    Zombie_new(tcbCNodeEntries, ZombieType_ZombieTCB, cte_ptr.get_ptr());
                fc_ret.cleanupInfo = cap_null_cap::new().unsplay();
                return fc_ret;
            }
        }
        #[cfg(feature = "KERNEL_MCS")]
        cap_tag::cap_sched_context_cap => {
            if _final {
                let sc = convert_to_mut_type_ref::<sched_context_t>(
                    cap::cap_sched_context_cap(capability).get_capSCPtr() as usize,
                );
                sc.schedContext_unbindAllTCBs();
                sc.schedContext_unbindNtfn();
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
                deletingIRQHandler(irq);
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
        setIRQState(IRQState::IRQInactive, irq);
    }
}

#[no_mangle]
pub fn preemptionPoint() -> exception_t {
    unsafe {
        ksWorkUnitsCompleted += 1;
        if ksWorkUnitsCompleted >= CONFIG_MAX_NUM_WORK_UNITS_PER_PREEMPTION {
            ksWorkUnitsCompleted = 0;

            #[cfg(feature = "KERNEL_MCS")]
            {
                updateTimestamp();
                let sc = convert_to_mut_type_ref::<sched_context_t>(ksCurSC);
                if !(sc.sc_active() && sc.refill_sufficient(ksConsumed))
                    || isCurDomainExpired()
                    || isIRQPending()
                {
                    return exception_t::EXCEPTION_PREEMTED;
                }
            }
            #[cfg(not(feature = "KERNEL_MCS"))]
            if isIRQPending() {
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
            &get_currenct_thread().get_cspace(tcbVTable).capability,
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
            &get_currenct_thread().get_cspace(tcbVTable).capability,
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
            &get_currenct_thread().get_cspace(tcbVTable).capability,
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
            &get_currenct_thread().get_cspace(tcbVTable).capability,
        ) {
            current_lookup_fault = lookup_fault;
        }
    }
}
