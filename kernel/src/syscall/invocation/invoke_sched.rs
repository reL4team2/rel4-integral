use sel4_common::{
    structures::exception_t,
    structures_gen::{cap, cap_tag},
    utils::convert_to_mut_type_ref,
};
use sel4_task::{sched_context::sched_context, tcb_t};

pub fn invokeSchedContext_UnbindObject(sc: &mut sched_context, capability: cap) -> exception_t {
    match capability.get_tag() {
        cap_tag::cap_thread_cap => {
            sc.schedContext_unbindTCB(convert_to_mut_type_ref::<tcb_t>(sc.scTcb));
        }
        cap_tag::cap_notification_cap => {
            sc.schedContext_unbindNtfn();
        }
        _ => {
            panic!("invalid cap type");
        }
    }
    exception_t::EXCEPTION_NONE
}

pub fn invokeSchedContext_Bind(sc: &mut sched_context) -> exception_t {
    // TODO: MCS
    exception_t::EXCEPTION_NONE
}
pub fn invokeSchedContext_Unbind(sc: &mut sched_context) -> exception_t {
    // TODO: MCS
    exception_t::EXCEPTION_NONE
}
pub fn invokeSchedContext_Consumed(sc: &mut sched_context) -> exception_t {
    // TODO: MCS
    exception_t::EXCEPTION_NONE
}
pub fn invokeSchedContext_YieldTo(sc: &mut sched_context) -> exception_t {
    // TODO: MCS
    exception_t::EXCEPTION_NONE
}
