use core::intrinsics::unlikely;

use log::debug;
use sel4_common::{
    arch::{us_to_ticks, MessageLabel},
    platform::time_def::time_t,
    sel4_config::{
        SEL4_ILLEGAL_OPERATION, SEL4_INVALID_CAPABILITY, SEL4_RANGE_ERROR, SEL4_TRUNCATED_MESSAGE,
        TIME_ARG_SIZE,
    },
    structures::{exception_t, seL4_IPCBuffer},
    structures_gen::{
        cap, cap_Splayed, cap_sched_context_cap, cap_sched_control_cap, cap_tag, notification_t,
    },
    utils::{convert_to_mut_type_ref, global_ops},
};
use sel4_cspace::interface::cte_t;
use sel4_task::{
    get_currenct_thread, get_currenct_thread_raw,
    sched_context::{
        max_period_us, min_budget, min_budget_us, refill_absolute_max, sched_context,
        sched_context_t, MIN_REFILLS,
    },
    set_thread_state, tcb_t, ThreadState,
};

use crate::{
    kernel::boot::{current_extra_caps, current_syscall_error, get_extra_cap_by_index},
    syscall::{
        get_syscall_arg,
        invocation::invoke_sched::{
            invokeSchedContext_UnbindObject, invoke_sched_context_bind,
            invoke_sched_context_consumed, invoke_sched_context_unbind,
            invoke_sched_context_yield_to, invoke_sched_control_configure_flags,
        },
    },
};

pub fn decode_sched_context_invocation(
    inv_label: MessageLabel,
    capability: &cap_sched_context_cap,
) -> exception_t {
    // sel4_common::println!("go into decode sched context invocation");
    let sc = convert_to_mut_type_ref::<sched_context_t>(capability.get_capSCPtr() as usize);
    
    #[cfg(feature = "enable_smp")]
    {
        if sc.scTcb != 0 {
            crate::smp::ipi::remote_tcb_stall(convert_to_mut_type_ref::<tcb_t>(sc.scTcb));
        }
    }
    
    match inv_label {
        MessageLabel::SchedContextConsumed => {
            set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
            invoke_sched_context_consumed(sc)
        }
        MessageLabel::SchedContextBind => decode_sched_context_bind(sc),
        MessageLabel::SchedContextUnbindObject => decode_sched_context_unbind_object(sc),
        MessageLabel::SchedContextUnbind => {
            if sc.scTcb == get_currenct_thread_raw() {
                debug!("SchedContext UnbindObject: cannot unbind sc of current thread");
                unsafe {
                    current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
            set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
            invoke_sched_context_unbind(sc)
        }
        MessageLabel::SchedContextYieldTo => decode_sched_context_yield_to(sc),
        _ => {
            debug!("SchedContext invocation: Illegal operation attempted.");
            unsafe {
                current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
            }
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }
    }
}
pub fn decode_sched_control_invocation(
    inv_label: MessageLabel,
    length: usize,
    capability: &cap_sched_control_cap,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    match inv_label {
        MessageLabel::SchedControlConfigureFlags => {
            if global_ops!(current_extra_caps.excaprefs[0] == 0) {
                debug!("SchedControl_ConfigureFlags: Truncated message.");
                unsafe {
                    current_syscall_error._type = SEL4_TRUNCATED_MESSAGE;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }

            if length < (TIME_ARG_SIZE * 2) + 3 {
                debug!("SchedControl_configureFlags: truncated message.");
                unsafe {
                    current_syscall_error._type = SEL4_TRUNCATED_MESSAGE;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }

            let budget_us: time_t = get_syscall_arg(0, buffer);
            let budget_ticks = us_to_ticks(budget_us);
            let period_us = get_syscall_arg(TIME_ARG_SIZE, buffer);
            let period_ticks = us_to_ticks(period_us);
            let extra_refills = get_syscall_arg(TIME_ARG_SIZE * 2, buffer);
            let badge = get_syscall_arg(TIME_ARG_SIZE * 2 + 1, buffer);
            let flags = get_syscall_arg(TIME_ARG_SIZE * 2 + 2, buffer);

            let targetCap =
                &convert_to_mut_type_ref::<cte_t>(unsafe { current_extra_caps.excaprefs[0] })
                    .capability;
            if unlikely(targetCap.get_tag() != cap_tag::cap_sched_context_cap) {
                debug!("SchedControl_ConfigureFlags: target cap not a scheduling context cap");
                unsafe {
                    current_syscall_error._type = SEL4_INVALID_CAPABILITY;
                    current_syscall_error.invalidCapNumber = 1;
                    return exception_t::EXCEPTION_SYSCALL_ERROR;
                }
            }
            if budget_us > max_period_us() || budget_ticks < min_budget() {
                debug!("SchedControl_ConfigureFlags: budget out of range.");
                unsafe {
                    current_syscall_error._type = SEL4_RANGE_ERROR;
                    current_syscall_error.rangeErrorMin = min_budget_us();
                    current_syscall_error.rangeErrorMax = max_period_us();
                }

                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }

            if period_us > max_period_us() || period_ticks < min_budget() {
                debug!("SchedControl_ConfigureFlags: period out of range.");
                unsafe {
                    current_syscall_error._type = SEL4_RANGE_ERROR;
                    current_syscall_error.rangeErrorMin = min_budget_us();
                    current_syscall_error.rangeErrorMax = max_period_us();
                }

                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }

            if budget_ticks > period_ticks {
                debug!("SchedControl_ConfigureFlags: budget must be <= period");
                unsafe {
                    current_syscall_error._type = SEL4_RANGE_ERROR;
                    current_syscall_error.rangeErrorMin = min_budget_us();
                    current_syscall_error.rangeErrorMax = period_us;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }

            if extra_refills + MIN_REFILLS
                > refill_absolute_max(cap::cap_sched_context_cap(&targetCap))
            {
                unsafe {
                    current_syscall_error._type = SEL4_RANGE_ERROR;
                    current_syscall_error.rangeErrorMin = 0;
                    current_syscall_error.rangeErrorMax =
                        refill_absolute_max(cap::cap_sched_context_cap(&targetCap)) - MIN_REFILLS;
                    debug!(
                        "Max refills invalid, got {}, max {}",
                        extra_refills, current_syscall_error.rangeErrorMax
                    );
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
            set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
            return invoke_sched_control_configure_flags(
                convert_to_mut_type_ref::<sched_context_t>(
                    cap::cap_sched_context_cap(&targetCap).get_capSCPtr() as usize,
                ),
                capability.get_core() as usize,
                budget_ticks,
                period_ticks,
                extra_refills + MIN_REFILLS,
                badge,
                flags,
            );
        }
        _ => {
            debug!("SchedControl invocation: Illegal operation attempted.");
            unsafe {
                current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
            }
        }
    }
    exception_t::EXCEPTION_NONE
}
pub fn decode_sched_context_unbind_object(sc: &mut sched_context) -> exception_t {
    // TODO: MCS
    if get_extra_cap_by_index(0).is_none() {
        debug!("SchedContext_Unbind: Truncated message.");
        unsafe {
            current_syscall_error._type = SEL4_TRUNCATED_MESSAGE;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    let capability = &get_extra_cap_by_index(0).unwrap().capability;
    match capability.clone().splay() {
        cap_Splayed::thread_cap(data) => {
            if sc.scTcb != data.get_capTCBPtr() as usize {
                debug!("SchedContext UnbindObject: object not bound");
                unsafe {
                    current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
            if sc.scTcb == get_currenct_thread_raw() {
                debug!("SchedContext UnbindObject: cannot unbind sc of current thread");
                unsafe {
                    current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
        }
        cap_Splayed::notification_cap(data) => {
            if sc.scNotification != data.get_capNtfnPtr() as usize {
                debug!("SchedContext UnbindObject: object not bound");
                unsafe {
                    current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
        }
        _ => {
            debug!("SchedContext_Unbind: invalid cap");
            unsafe {
                current_syscall_error._type = SEL4_INVALID_CAPABILITY;
                current_syscall_error.invalidCapNumber = 1;
            }
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }
    }
    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    return invokeSchedContext_UnbindObject(sc, capability.clone());
}
pub fn decode_sched_context_bind(sc: &mut sched_context) -> exception_t {
    if get_extra_cap_by_index(0).is_none() {
        debug!("SchedContext_Bind: Truncated Message.");
        unsafe {
            current_syscall_error._type = SEL4_TRUNCATED_MESSAGE;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    let capability = &get_extra_cap_by_index(0).unwrap().capability;
    match capability.clone().splay() {
        cap_Splayed::thread_cap(data) => {
            if sc.scTcb != 0 {
                debug!("SchedContext_Bind: sched context already bound.");
                unsafe {
                    current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }

            if convert_to_mut_type_ref::<tcb_t>(data.get_capTCBPtr() as usize).tcbSchedContext != 0
            {
                debug!("SchedContext_Bind: tcb already bound.");
                unsafe {
                    current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }

            if convert_to_mut_type_ref::<tcb_t>(data.get_capTCBPtr() as usize).is_blocked()
                && !sc.sc_released()
            {
                debug!("SchedContext_Bind: tcb blocked and scheduling context not schedulable.");
                unsafe {
                    current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
            set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
            return invoke_sched_context_bind(sc, &capability);
        }
        cap_Splayed::notification_cap(data) => {
            if sc.scNotification != 0 {
                debug!("SchedContext_Bind: sched context already bound.");
                unsafe {
                    current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
            if convert_to_mut_type_ref::<notification_t>(data.get_capNtfnPtr() as usize)
                .get_ntfnSchedContext()
                != 0
            {
                debug!("SchedContext_Bind: notification already bound");
                unsafe {
                    current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
            set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
            return invoke_sched_context_bind(sc, &capability);
        }
        _ => {
            debug!("SchedContext_Bind: invalid cap.");
            unsafe {
                current_syscall_error._type = SEL4_INVALID_CAPABILITY;
                current_syscall_error.invalidCapNumber = 1;
            }
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }
    }
}
pub fn decode_sched_context_yield_to(sc: &mut sched_context) -> exception_t {
    let thread = get_currenct_thread();

    if sc.scTcb == 0 {
        debug!("SchedContext_YieldTo: cannot yield to an inactive sched context");
        unsafe {
            current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    if sc.scTcb == thread.get_ptr() {
        debug!("SchedContext_YieldTo: cannot seL4_SchedContext_YieldTo on self");
        unsafe {
            current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    if convert_to_mut_type_ref::<tcb_t>(sc.scTcb).tcbPriority > thread.tcbMCP {
        debug!(
            "SchedContext_YieldTo: insufficient mcp {} to yield to a thread with prio {}",
            thread.tcbMCP,
            convert_to_mut_type_ref::<tcb_t>(sc.scTcb).tcbPriority
        );
        unsafe {
            current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    assert!(thread.tcbYieldTo == 0);
    if thread.tcbYieldTo != 0 {
        debug!(
            "SchedContext_YieldTo: cannot seL4_SchedContext_YieldTo to more than on SC at a time"
        );
        unsafe {
            current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    set_thread_state(thread, ThreadState::ThreadStateRestart);
    return invoke_sched_context_yield_to(sc);
}
