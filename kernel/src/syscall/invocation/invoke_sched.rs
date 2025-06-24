#[cfg(feature = "enable_smp")]
use sel4_common::arch::get_current_cpu_index;
use sel4_common::{
    platform::time_def::ticks_t,
    structures::exception_t,
    structures_gen::{call_stack, cap, cap_Splayed, cap_tag, notification_t},
    utils::{convert_to_mut_type_ref, convert_to_option_mut_type_ref},
};
use sel4_task::{
    check_budget, commit_time, get_currenct_thread, possible_switch_to,
    reply::reply_t,
    reschedule_required,
    sched_context::{sched_context, MIN_REFILLS},
    tcb_t, NODE_STATE, SCHED_CONTEXT_SPORADIC,
};

pub fn invokeSchedContext_UnbindObject(sc: &mut sched_context, capability: cap) -> exception_t {
    match capability.get_tag() {
        cap_tag::cap_thread_cap => {
            sc.sched_context_unbind_tcb(convert_to_mut_type_ref::<tcb_t>(sc.scTcb));
        }
        cap_tag::cap_notification_cap => {
            sc.sched_context_unbind_ntfn();
        }
        _ => {
            panic!("invalid cap type");
        }
    }
    exception_t::EXCEPTION_NONE
}

pub fn invoke_sched_context_bind(sc: &mut sched_context, capability: &cap) -> exception_t {
    match capability.clone().splay() {
        cap_Splayed::thread_cap(data) => sc.sched_context_bind_tcb(
            convert_to_mut_type_ref::<tcb_t>(data.get_capTCBPtr() as usize),
        ),
        cap_Splayed::notification_cap(data) => sc.sched_context_bind_ntfn(
            convert_to_mut_type_ref::<notification_t>(data.get_capNtfnPtr() as usize),
        ),
        _ => {
            panic!("invalid cap type of invoke sched context bind")
        }
    }
    exception_t::EXCEPTION_NONE
}
pub fn invoke_sched_context_unbind(sc: &mut sched_context) -> exception_t {
    sc.sched_context_unbind_all_tcbs();
    sc.sched_context_unbind_ntfn();
    if sc.scReply != 0 {
        convert_to_mut_type_ref::<reply_t>(sc.scReply).replyNext = call_stack::new(0, 0);
        sc.scReply = 0;
    }
    exception_t::EXCEPTION_NONE
}
pub fn invoke_sched_context_consumed(sc: &mut sched_context) -> exception_t {
    // TODO: MCS
    sc.set_consumed();
    exception_t::EXCEPTION_NONE
}
pub fn invoke_sched_context_yield_to(sc: &mut sched_context) -> exception_t {
    if sc.scYieldFrom != 0 {
        convert_to_mut_type_ref::<tcb_t>(sc.scYieldFrom).schedContext_completeYieldTo();
        assert!(sc.scYieldFrom == 0);
    }
    sc.sched_context_resume();
    let mut return_now = true;
    let tcb = convert_to_mut_type_ref::<tcb_t>(sc.scTcb);
    #[cfg(feature = "enable_smp")]
    if tcb.is_schedulable() {
        if sc.scCore != get_current_cpu_index()
            || tcb.tcbPriority < get_currenct_thread().tcbPriority
        {
            tcb.sched_dequeue();
            tcb.sched_enqueue();
        } else {
            get_currenct_thread().tcbYieldTo = sc.get_ptr();
            sc.scYieldFrom = get_currenct_thread().get_ptr();
            tcb.sched_dequeue();
            get_currenct_thread().sched_enqueue();
            tcb.sched_enqueue();
            reschedule_required();
            return_now = false;
        }
    }

    #[cfg(not(feature = "enable_smp"))]
    if tcb.is_schedulable() {
        if tcb.tcbPriority < get_currenct_thread().tcbPriority {
            tcb.sched_dequeue();
            tcb.sched_enqueue();
        } else {
            get_currenct_thread().tcbYieldTo = sc.get_ptr();
            sc.scYieldFrom = get_currenct_thread().get_ptr();
            tcb.sched_dequeue();
            get_currenct_thread().sched_enqueue();
            tcb.sched_enqueue();
            reschedule_required();
            return_now = false;
        }
    }
    if return_now == true {
        sc.set_consumed();
    }
    exception_t::EXCEPTION_NONE
}
pub fn invoke_sched_control_configure_flags(
    target: &mut sched_context,
    _core: usize,
    budget: ticks_t,
    period: ticks_t,
    max_refills: usize,
    badge: usize,
    flags: usize,
) -> exception_t {
    target.scBadge = badge;
    target.scSporadic = (flags & SCHED_CONTEXT_SPORADIC) != 0;

    if let Some(tcb) = convert_to_option_mut_type_ref::<tcb_t>(target.scTcb) {
        #[cfg(feature = "enable_smp")]
        crate::smp::ipi::remote_tcb_stall(tcb);
        /* remove from scheduler */
        tcb.release_remove();
        tcb.sched_dequeue();
        /* bill the current consumed amount before adjusting the params */
        if target.is_current() {
            assert!(check_budget());
            commit_time();
        }
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "enable_smp")] {
            if budget == period {
                target.refill_new(MIN_REFILLS, budget, 0);
            } else if target.scRefillMax > 0
                && target.scTcb != 0
                && convert_to_mut_type_ref::<tcb_t>(target.scTcb).is_runnable()
                && _core == target.scCore
            {
                target.refill_update(period, budget, max_refills);
            } else {
                /* the scheduling context isn't active - it's budget is not being used, so
                 * we can just populate the parameters from now */
                target.refill_new(max_refills, budget, period);
            }
        } else {
            if budget == period {
                target.refill_new(MIN_REFILLS, budget, 0);
            } else if target.scRefillMax > 0
                && target.scTcb != 0
                && convert_to_mut_type_ref::<tcb_t>(target.scTcb).is_runnable()
            {
                target.refill_update(period, budget, max_refills);
            } else {
                /* the scheduling context isn't active - it's budget is not being used, so
                 * we can just populate the parameters from now */
                target.refill_new(max_refills, budget, period);
            }
        }
    }

    #[cfg(feature = "enable_smp")]
    {
        target.scCore = _core;
        if let Some(tcb) = convert_to_option_mut_type_ref::<tcb_t>(target.scTcb) {
            crate::smp::migrate_tcb(tcb, target.scCore);
        }
    }

    assert!(target.scRefillMax > 0);
    if target.scTcb != 0 {
        target.sched_context_resume();
        if _core == sel4_common::utils::cpu_id() {
            if convert_to_mut_type_ref::<tcb_t>(target.scTcb).is_runnable()
                && target.scTcb != NODE_STATE!(ksCurThread)
            {
                possible_switch_to(convert_to_mut_type_ref::<tcb_t>(target.scTcb));
            }
        } else {
            if let Some(tcb) = convert_to_option_mut_type_ref::<tcb_t>(target.scTcb) {
                if tcb.is_runnable() {
                    tcb.sched_enqueue();
                }
            }
        }
        if target.scTcb == NODE_STATE!(ksCurThread) {
            reschedule_required();
        }
    }
    exception_t::EXCEPTION_NONE
}
