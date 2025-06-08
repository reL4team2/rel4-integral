pub mod invocation;
pub mod syscall_reply;
pub mod utils;

use super::arch::handle_unknown_syscall;
use core::intrinsics::unlikely;
use sel4_common::arch::ArchReg;
// use sel4_common::ffi_call;
#[cfg(feature = "kernel_mcs")]
use sel4_common::arch::ArchReg::*;
#[cfg(not(feature = "kernel_mcs"))]
use sel4_common::sel4_config::TCB_CALLER;

pub const SYS_CALL: isize = -1;
pub const SYSCALL_MAX: isize = SYS_CALL;
pub const SYS_REPLY_RECV: isize = -2;

#[cfg(not(feature = "kernel_mcs"))]
pub const SYS_SEND: isize = -3;
#[cfg(not(feature = "kernel_mcs"))]
pub const SYS_NB_SEND: isize = -4;
#[cfg(not(feature = "kernel_mcs"))]
pub const SYS_RECV: isize = -5;
#[cfg(not(feature = "kernel_mcs"))]
pub const SYS_REPLY: isize = -6;
#[cfg(not(feature = "kernel_mcs"))]
pub const SYS_YIELD: isize = -7;

#[cfg(feature = "kernel_mcs")]
pub const SYS_NB_SEND_RECV: isize = -3;
#[cfg(feature = "kernel_mcs")]
pub const SYS_NB_SEND_WAIT: isize = -4;
#[cfg(feature = "kernel_mcs")]
pub const SYS_SEND: isize = -5;
#[cfg(feature = "kernel_mcs")]
pub const SYS_NB_SEND: isize = -6;
#[cfg(feature = "kernel_mcs")]
pub const SYS_RECV: isize = -7;

pub const SYS_NB_RECV: isize = -8;

#[cfg(feature = "kernel_mcs")]
pub const SYS_WAIT: isize = -9;
#[cfg(feature = "kernel_mcs")]
pub const SYS_NB_WAIT: isize = -10;
#[cfg(feature = "kernel_mcs")]
pub const SYS_YIELD: isize = -11;
#[cfg(feature = "kernel_mcs")]
pub const SYSCALL_MIN: isize = SYS_YIELD;
#[cfg(not(feature = "kernel_mcs"))]
pub const SYSCALL_MIN: isize = SYS_NB_RECV;

pub const SYS_DEBUG_PUT_CHAR: isize = SYSCALL_MIN - 1;
pub const SYS_DEBUG_DUMP_SCHEDULER: isize = SYS_DEBUG_PUT_CHAR - 1;
pub const SYS_DEBUG_HALT: isize = SYS_DEBUG_DUMP_SCHEDULER - 1;
pub const SYS_DEBUG_CAP_IDENTIFY: isize = SYS_DEBUG_HALT - 1;
pub const SYS_DEBUG_SNAPSHOT: isize = SYS_DEBUG_CAP_IDENTIFY - 1;
pub const SYS_DEBUG_NAME_THREAD: isize = SYS_DEBUG_SNAPSHOT - 1;
#[cfg(not(feature = "kernel_mcs"))]
pub const SYS_GET_CLOCK: isize = -30;
#[cfg(feature = "kernel_mcs")]
pub const SYS_GET_CLOCK: isize = -33;
#[cfg(feature = "kernel_mcs")]
use crate::structures::lookupCap_ret_t;
use sel4_common::structures::exception_t;
use sel4_common::structures_gen::{
    cap, cap_Splayed, cap_tag, endpoint, lookup_fault_missing_capability, notification,
    seL4_Fault_CapFault,
};
use sel4_common::utils::convert_to_mut_type_ref;
#[cfg(not(feature = "kernel_mcs"))]
use sel4_common::utils::ptr_to_mut;
#[cfg(not(feature = "kernel_mcs"))]
use sel4_ipc::Transfer;
use sel4_ipc::{endpoint_func, notification_func};
#[cfg(not(feature = "kernel_mcs"))]
use sel4_task::reschedule_required;
use sel4_task::{
    activateThread, get_currenct_thread, schedule, set_thread_state, tcb_t, ThreadState,
};
#[cfg(feature = "kernel_mcs")]
use sel4_task::{charge_budget, get_current_sc, mcs_preemption_point, NODE_STATE};
pub use utils::*;

use crate::arch::restore_user_context;
use crate::interrupt::get_active_irq;
use crate::interrupt::handler::handle_interrput;
use crate::kernel::boot::current_lookup_fault;
use sel4_common::ffi::current_fault;
use sel4_common::platform::IRQ_INVALID;

use self::invocation::handle_invocation;

#[no_mangle]
pub fn slowpath(syscall: usize) {
    if (syscall as isize) < SYSCALL_MIN || (syscall as isize) > SYSCALL_MAX {
        // using ffi_call! macro to call c function
        handle_unknown_syscall(syscall as isize);
        // ffi_call!(handle_unknown_syscall(id: usize => syscall));
    } else {
        handlesyscall(syscall);
    }
    restore_user_context();
}

#[no_mangle]
#[cfg(not(feature = "kernel_mcs"))]
pub fn handlesyscall(_syscall: usize) -> exception_t {
    let syscall: isize = _syscall as isize;
    // if hart_id() == 0 {
    //     debug!("handle syscall: {}", syscall);
    // }
    // sel4_common::println!("handle syscall {}", syscall);
    match syscall {
        SYS_SEND => {
            let ret = handle_invocation(false, true);
            if unlikely(ret != exception_t::EXCEPTION_NONE) {
                let irq = get_active_irq();
                if irq != IRQ_INVALID {
                    handle_interrput(irq);
                }
            }
        }
        SYS_NB_SEND => {
            let ret = handle_invocation(false, false);
            if unlikely(ret != exception_t::EXCEPTION_NONE) {
                let irq = get_active_irq();
                if irq != IRQ_INVALID {
                    handle_interrput(irq);
                }
            }
        }
        SYS_CALL => {
            let ret = handle_invocation(true, true);
            if unlikely(ret != exception_t::EXCEPTION_NONE) {
                let irq = get_active_irq();
                if irq != IRQ_INVALID {
                    handle_interrput(irq);
                }
            }
        }
        SYS_RECV => {
            handle_recv(true);
        }
        SYS_REPLY => handle_reply(),
        SYS_REPLY_RECV => {
            handle_reply();
            handle_recv(true);
        }
        SYS_NB_RECV => handle_recv(false),
        SYS_YIELD => handle_yield(),
        _ => panic!("Invalid syscall"),
    }
    schedule();
    activateThread();
    exception_t::EXCEPTION_NONE
}
#[no_mangle]
#[cfg(feature = "kernel_mcs")]
pub fn handlesyscall(_syscall: usize) -> exception_t {
    use core::intrinsics::likely;
    use sel4_task::{check_budget_restart, update_timestamp};

    let syscall: isize = _syscall as isize;
    // if hart_id() == 0 {
    //     debug!("handle syscall: {}", syscall);
    // }
    // sel4_common::println!("handle syscall {}", syscall);
    update_timestamp();
    if likely(check_budget_restart()) {
        match syscall {
            SYS_SEND => {
                let ret = handle_invocation(
                    false,
                    true,
                    false,
                    false,
                    get_currenct_thread().tcbArch.get_register(Cap),
                );
                if unlikely(ret != exception_t::EXCEPTION_NONE) {
                    mcs_preemption_point();
                    let irq = get_active_irq();
                    if irq != IRQ_INVALID {
                        handle_interrput(irq);
                    }
                }
            }
            SYS_NB_SEND => {
                let ret = handle_invocation(
                    false,
                    false,
                    false,
                    false,
                    get_currenct_thread().tcbArch.get_register(Cap),
                );
                if unlikely(ret != exception_t::EXCEPTION_NONE) {
                    mcs_preemption_point();
                    let irq = get_active_irq();
                    if irq != IRQ_INVALID {
                        handle_interrput(irq);
                    }
                }
            }
            SYS_CALL => {
                let ret = handle_invocation(
                    true,
                    true,
                    true,
                    false,
                    get_currenct_thread().tcbArch.get_register(Cap),
                );
                if unlikely(ret != exception_t::EXCEPTION_NONE) {
                    mcs_preemption_point();
                    let irq = get_active_irq();
                    if irq != IRQ_INVALID {
                        handle_interrput(irq);
                    }
                }
            }
            SYS_RECV => {
                handle_recv(true, true);
            }
            SYS_WAIT => {
                handle_recv(true, false);
            }
            SYS_NB_WAIT => {
                handle_recv(false, false);
            }
            SYS_REPLY_RECV => {
                let reply = get_currenct_thread().tcbArch.get_register(Reply);
                let ret = handle_invocation(false, false, true, true, reply);
                assert!(ret == exception_t::EXCEPTION_NONE);
                handle_recv(true, true);
            }
            SYS_NB_SEND_RECV => {
                // TODO: MCS
                let dest = get_currenct_thread().tcbArch.get_register(nbsRecvDest);
                let ret = handle_invocation(false, false, true, true, dest);
                if unlikely(ret != exception_t::EXCEPTION_NONE) {
                    mcs_preemption_point();
                    let irq = get_active_irq();
                    if irq != IRQ_INVALID {
                        handle_interrput(irq);
                    }
                } else {
                    handle_recv(true, true);
                }
            }
            SYS_NB_SEND_WAIT => {
                let reply = get_currenct_thread().tcbArch.get_register(Reply);
                let ret = handle_invocation(false, false, true, true, reply);
                if unlikely(ret != exception_t::EXCEPTION_NONE) {
                    mcs_preemption_point();
                    let irq = get_active_irq();
                    if irq != IRQ_INVALID {
                        handle_interrput(irq);
                    }
                } else {
                    handle_recv(true, false);
                }
            }
            SYS_NB_RECV => handle_recv(false, true),
            SYS_YIELD => handle_yield(),
            _ => panic!("Invalid syscall"),
        }
    }
    schedule();
    activateThread();
    exception_t::EXCEPTION_NONE
}
#[cfg(feature = "kernel_mcs")]
fn send_fault_ipc(thread: &mut tcb_t, handlerCap: &cap, can_donate: bool) -> bool {
    // TODO: MCS
    if handlerCap.get_tag() == cap_tag::cap_endpoint_cap {
        assert!(cap::cap_endpoint_cap(&handlerCap).get_capCanSend() != 0);
        assert!(
            cap::cap_endpoint_cap(&handlerCap).get_capCanGrant() != 0
                || cap::cap_endpoint_cap(&handlerCap).get_capCanGrantReply() != 0
        );
        thread.tcbFault = unsafe { current_fault.clone() };
        convert_to_mut_type_ref::<endpoint>(
            cap::cap_endpoint_cap(&handlerCap).get_capEPPtr() as usize
        )
        .send_ipc(
            thread,
            true,
            false,
            cap::cap_endpoint_cap(&handlerCap).get_capCanGrant() != 0,
            cap::cap_endpoint_cap(&handlerCap).get_capEPBadge() as usize,
            cap::cap_endpoint_cap(&handlerCap).get_capCanGrantReply() != 0,
            can_donate,
        );
        return true;
    } else {
        assert!(handlerCap.get_tag() == cap_tag::cap_null_cap);
        return false;
    }
}
#[cfg(not(feature = "kernel_mcs"))]
fn send_fault_ipc(thread: &mut tcb_t) -> exception_t {
    use sel4_common::structures_gen::seL4_Fault_tag;

    let origin_lookup_fault = unsafe { current_lookup_fault.clone() };
    let lu_ret = thread.lookup_slot(thread.TCB_FAULT_HANDLER);
    if lu_ret.status != exception_t::EXCEPTION_NONE {
        unsafe {
            current_fault = seL4_Fault_CapFault::new(thread.TCB_FAULT_HANDLER as u64, 0).unsplay();
        }
        return exception_t::EXCEPTION_FAULT;
    }

    if ptr_to_mut(lu_ret.slot).capability.clone().get_tag() == cap_tag::cap_endpoint_cap
        && (cap::cap_endpoint_cap(&ptr_to_mut(lu_ret.slot).capability).get_capCanGrant() != 0
            || cap::cap_endpoint_cap(&ptr_to_mut(lu_ret.slot).capability).get_capCanGrantReply()
                != 0)
    {
        let handler_cap = cap::cap_endpoint_cap(&ptr_to_mut(lu_ret.slot).capability);
        thread.tcbFault = unsafe { current_fault.clone() };
        if thread.tcbFault.get_tag() == seL4_Fault_tag::seL4_Fault_CapFault {
            thread.tcbLookupFailure = origin_lookup_fault;
        }
        convert_to_mut_type_ref::<endpoint>(handler_cap.get_capEPPtr() as usize).send_ipc(
            thread,
            true,
            true,
            handler_cap.get_capCanGrant() != 0,
            handler_cap.get_capEPBadge() as usize,
            true,
        );
    } else {
        unsafe {
            current_fault = seL4_Fault_CapFault::new(thread.TCB_FAULT_HANDLER as u64, 0).unsplay();
            current_lookup_fault = lookup_fault_missing_capability::new(0).unsplay();
        }
        return exception_t::EXCEPTION_FAULT;
    }
    exception_t::EXCEPTION_NONE
}

#[inline]
#[cfg(not(feature = "kernel_mcs"))]
pub fn handle_fault(thread: &mut tcb_t) {
    if send_fault_ipc(thread) != exception_t::EXCEPTION_NONE {
        set_thread_state(thread, ThreadState::ThreadStateInactive);
    }
}
#[inline]
#[cfg(feature = "kernel_mcs")]
pub fn handle_fault(thread: &mut tcb_t) {
    use sel4_common::sel4_config::TCB_FAULT_HANDLER;
    let cte = thread.get_cspace(TCB_FAULT_HANDLER);
    let hasFaultHandler = send_fault_ipc(thread, &cte.capability, thread.tcbSchedContext != 0);
    if !hasFaultHandler {
        set_thread_state(thread, ThreadState::ThreadStateInactive);
    }
}
#[inline]
#[cfg(feature = "kernel_mcs")]
#[no_mangle]
pub fn handleTimeout(tptr: &mut tcb_t) {
    use sel4_common::sel4_config::TCB_TIMEOUT_HANDLER;

    assert!(tptr.valid_timeout_handler());
    let cte = tptr.get_cspace(TCB_TIMEOUT_HANDLER);
    send_fault_ipc(tptr, &cte.capability, false);
}
#[inline]
#[cfg(feature = "kernel_mcs")]
#[no_mangle]
pub fn endTimeslice(can_timeout_fault: bool) {
    use sel4_common::structures_gen::seL4_Fault_Timeout;

    unsafe {
        let thread = get_currenct_thread();
        let sched_context = get_current_sc();
        if can_timeout_fault && !sched_context.is_round_robin() && thread.valid_timeout_handler() {
            current_fault = seL4_Fault_Timeout::new(sched_context.scBadge as u64).unsplay();
            handleTimeout(thread);
        } else if sched_context.refill_ready() && sched_context.refill_sufficient(0) {
            /* apply round robin */
            assert!(sched_context.refill_sufficient(0));
            assert!(thread.tcbState.get_tcbQueued() == 0);
            thread.sched_append();
        } else {
            /* postpone until ready */
            sched_context.postpone();
        }
    }
}
#[cfg(feature = "kernel_mcs")]
#[inline]
pub fn lookup_reply() -> lookupCap_ret_t {
    use log::debug;

    use crate::object::lookup_cap;

    let reply_ptr = get_currenct_thread().tcbArch.get_register(ArchReg::Reply);
    let mut lu_ret = lookup_cap(get_currenct_thread(), reply_ptr);

    if unlikely(lu_ret.status != exception_t::EXCEPTION_NONE) {
        debug!("Reply cap lookup failed");
        unsafe { current_fault = seL4_Fault_CapFault::new(reply_ptr as u64, 1).unsplay() };
        handle_fault(get_currenct_thread());
        return lu_ret;
    }

    if unlikely(lu_ret.capability.get_tag() != cap_tag::cap_reply_cap) {
        debug!("Cap in reply slot is not a reply");
        unsafe { current_fault = seL4_Fault_CapFault::new(reply_ptr as u64, 1).unsplay() };
        handle_fault(get_currenct_thread());
        lu_ret.status = exception_t::EXCEPTION_FAULT;
        return lu_ret;
    }
    lu_ret
}
// TODO: MCS
#[cfg(not(feature = "kernel_mcs"))]
fn handle_reply() {
    let current_thread = get_currenct_thread();
    let caller_slot = current_thread.get_cspace_mut_ref(TCB_CALLER);
    if caller_slot.capability.clone().get_tag() == cap_tag::cap_reply_cap {
        if cap::cap_reply_cap(&caller_slot.capability).get_capReplyMaster() != 0 {
            return;
        }
        let caller = convert_to_mut_type_ref::<tcb_t>(
            cap::cap_reply_cap(&caller_slot.capability).get_capTCBPtr() as usize,
        );
        current_thread.do_reply(
            caller,
            caller_slot,
            cap::cap_reply_cap(&caller_slot.capability).get_capReplyCanGrant() != 0,
        );
    }
}
#[cfg(feature = "kernel_mcs")]
fn handle_recv(block: bool, canReply: bool) {
    let current_thread = get_currenct_thread();
    let ep_cptr = current_thread.tcbArch.get_register(ArchReg::Cap);
    let lu_ret = current_thread.lookup_slot(ep_cptr);
    if lu_ret.status != exception_t::EXCEPTION_NONE {
        unsafe {
            current_fault = seL4_Fault_CapFault::new(ep_cptr as u64, 1).unsplay();
        }
        return handle_fault(current_thread);
    }
    let ipc_cap = unsafe { (*lu_ret.slot).capability.clone() };
    match ipc_cap.splay() {
        cap_Splayed::endpoint_cap(data) => {
            if unlikely(data.get_capCanReceive() == 0) {
                unsafe {
                    current_lookup_fault = lookup_fault_missing_capability::new(0).unsplay();
                    current_fault = seL4_Fault_CapFault::new(ep_cptr as u64, 1).unsplay();
                }
                return handle_fault(current_thread);
            }
            // TODO: MCS
            if canReply {
                let lu_ret = lookup_reply();
                if lu_ret.status != exception_t::EXCEPTION_NONE {
                    return;
                } else {
                    let reply_cap = lu_ret.capability;
                    convert_to_mut_type_ref::<endpoint>(data.get_capEPPtr() as usize).receive_ipc(
                        current_thread,
                        block,
                        Some(cap::cap_reply_cap(&reply_cap)),
                    );
                }
            } else {
                convert_to_mut_type_ref::<endpoint>(data.get_capEPPtr() as usize).receive_ipc(
                    current_thread,
                    block,
                    None,
                );
            }
        }

        cap_Splayed::notification_cap(data) => {
            let ntfn = convert_to_mut_type_ref::<notification>(data.get_capNtfnPtr() as usize);
            let bound_tcb_ptr = ntfn.get_ntfnBoundTCB();
            if unlikely(
                data.get_capNtfnCanReceive() == 0
                    || (bound_tcb_ptr != 0 && bound_tcb_ptr != current_thread.get_ptr() as u64),
            ) {
                unsafe {
                    current_lookup_fault = lookup_fault_missing_capability::new(0).unsplay();
                    current_fault = seL4_Fault_CapFault::new(ep_cptr as u64, 1).unsplay();
                }
                return handle_fault(current_thread);
            }
            return ntfn.receive_signal(current_thread, block);
        }
        _ => {
            unsafe {
                current_lookup_fault = lookup_fault_missing_capability::new(0).unsplay();
                current_fault = seL4_Fault_CapFault::new(ep_cptr as u64, 1).unsplay();
            }
            return handle_fault(current_thread);
        }
    }
}

#[cfg(not(feature = "kernel_mcs"))]
fn handle_recv(block: bool) {
    let current_thread = get_currenct_thread();
    let ep_cptr = current_thread.tcbArch.get_register(ArchReg::Cap);
    let lu_ret = current_thread.lookup_slot(ep_cptr);
    if lu_ret.status != exception_t::EXCEPTION_NONE {
        unsafe {
            current_fault = seL4_Fault_CapFault::new(ep_cptr as u64, 1).unsplay();
        }
        return handle_fault(current_thread);
    }
    let ipc_cap = unsafe { (*lu_ret.slot).capability.clone() };
    match ipc_cap.splay() {
        cap_Splayed::endpoint_cap(data) => {
            if unlikely(data.get_capCanReceive() == 0) {
                unsafe {
                    current_lookup_fault = lookup_fault_missing_capability::new(0).unsplay();
                    current_fault = seL4_Fault_CapFault::new(ep_cptr as u64, 1).unsplay();
                }
                return handle_fault(current_thread);
            }
            current_thread.delete_caller_cap();
            convert_to_mut_type_ref::<endpoint>(data.get_capEPPtr() as usize).receive_ipc(
                current_thread,
                block,
                data.get_capCanGrant() != 0,
            );
        }

        cap_Splayed::notification_cap(data) => {
            let ntfn = convert_to_mut_type_ref::<notification>(data.get_capNtfnPtr() as usize);
            let bound_tcb_ptr = ntfn.get_ntfnBoundTCB();
            if unlikely(
                data.get_capNtfnCanReceive() == 0
                    || (bound_tcb_ptr != 0 && bound_tcb_ptr != current_thread.get_ptr() as u64),
            ) {
                unsafe {
                    current_lookup_fault = lookup_fault_missing_capability::new(0).unsplay();
                    current_fault = seL4_Fault_CapFault::new(ep_cptr as u64, 1).unsplay();
                }
                return handle_fault(current_thread);
            }
            return ntfn.receive_signal(current_thread, block);
        }
        _ => {
            unsafe {
                current_lookup_fault = lookup_fault_missing_capability::new(0).unsplay();
                current_fault = seL4_Fault_CapFault::new(ep_cptr as u64, 1).unsplay();
            }
            return handle_fault(current_thread);
        }
    }
}

fn handle_yield() {
    #[cfg(feature = "kernel_mcs")]
    {
        unsafe {
            let consumed = get_current_sc().scConsumed + NODE_STATE!(ksConsumed);
            charge_budget((*get_current_sc().refill_head()).rAmount, false);
            get_current_sc().scConsumed = consumed;
        }
    }
    #[cfg(not(feature = "kernel_mcs"))]
    {
        // let thread = get_currenct_thread();
        // let thread_ptr = thread as *mut tcb_t as usize;
        // sel4_common::println!("{}: handle_yield: {:#x}, tcb queued: {}, state: {:?}", thread.get_cpu(), thread_ptr, thread.tcbState.get_tcbQueued(), thread.get_state());
        get_currenct_thread().sched_dequeue();
        get_currenct_thread().sched_append();

        reschedule_required();
    }
}
