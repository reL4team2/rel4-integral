use log::debug;
use sel4_common::{structures::exception_t, utils::global_ops};
use sel4_task::sched_context::sched_context;

use crate::kernel::boot::current_extra_caps;

pub fn decode_sched_context_invocation() -> exception_t {
    exception_t::EXCEPTION_NONE
}
pub fn decode_sched_control_invocation() -> exception_t {
    exception_t::EXCEPTION_NONE
}
pub fn decodeSchedContext_UnbindObject(sc: &mut sched_context) -> exception_t {
    // TODO: MCS
    unimplemented!("MCS");
    if global_ops!(current_extra_caps.excaprefs[0] == 0) {
        debug!("")
    }
    exception_t::EXCEPTION_NONE
}
pub fn decodeSchedContext_Bind(sc: &mut sched_context) -> exception_t {
    unimplemented!("MCS");
    // TODO: MCS
    exception_t::EXCEPTION_NONE
}
pub fn decodeSchedContext_YieldTo(sc: &mut sched_context) {
    unimplemented!("MCS");
    // TODO: MCS
}
