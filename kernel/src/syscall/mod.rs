pub mod invocation;
pub mod syscall_reply;
pub mod utils;

use super::arch::handleUnknownSyscall;
use core::intrinsics::unlikely;
use sel4_common::arch::ArchReg;
// use sel4_common::ffi_call;
#[cfg(feature = "KERNEL_MCS")]
use sel4_common::arch::ArchReg::*;
#[cfg(not(feature = "KERNEL_MCS"))]
use sel4_common::sel4_config::tcbCaller;

pub const SysCall: isize = -1;
pub const SYSCALL_MAX: isize = SysCall;
pub const SysReplyRecv: isize = -2;

#[cfg(not(feature = "KERNEL_MCS"))]
pub const SysSend: isize = -3;
#[cfg(not(feature = "KERNEL_MCS"))]
pub const SysNBSend: isize = -4;
#[cfg(not(feature = "KERNEL_MCS"))]
pub const SysRecv: isize = -5;
#[cfg(not(feature = "KERNEL_MCS"))]
pub const SysReply: isize = -6;
#[cfg(not(feature = "KERNEL_MCS"))]
pub const SysYield: isize = -7;

#[cfg(feature = "KERNEL_MCS")]
pub const SysNBSendRecv: isize = -3;
#[cfg(feature = "KERNEL_MCS")]
pub const SysNBSendWait: isize = -4;
#[cfg(feature = "KERNEL_MCS")]
pub const SysSend: isize = -5;
#[cfg(feature = "KERNEL_MCS")]
pub const SysNBSend: isize = -6;
#[cfg(feature = "KERNEL_MCS")]
pub const SysRecv: isize = -7;

pub const SysNBRecv: isize = -8;

#[cfg(feature = "KERNEL_MCS")]
pub const SysWait: isize = -9;
#[cfg(feature = "KERNEL_MCS")]
pub const SysNBWait: isize = -10;
#[cfg(feature = "KERNEL_MCS")]
pub const SysYield: isize = -11;
#[cfg(feature = "KERNEL_MCS")]
pub const SYSCALL_MIN: isize = SysYield;
#[cfg(not(feature = "KERNEL_MCS"))]
pub const SYSCALL_MIN: isize = SysNBRecv;

pub const SysDebugPutChar: isize = SYSCALL_MIN - 1;
pub const SysDebugDumpScheduler: isize = SysDebugPutChar - 1;
pub const SysDebugHalt: isize = SysDebugDumpScheduler - 1;
pub const SysDebugCapIdentify: isize = SysDebugHalt - 1;
pub const SysDebugSnapshot: isize = SysDebugCapIdentify - 1;
pub const SysDebugNameThread: isize = SysDebugSnapshot - 1;
#[cfg(not(feature = "KERNEL_MCS"))]
pub const SysGetClock: isize = -30;
#[cfg(feature = "KERNEL_MCS")]
pub const SysGetClock: isize = -33;
#[cfg(feature = "KERNEL_MCS")]
use crate::structures::lookupCap_ret_t;
use sel4_common::structures::exception_t;
use sel4_common::structures_gen::{
    cap, cap_Splayed, cap_tag, endpoint, lookup_fault_missing_capability, notification,
    seL4_Fault_CapFault,
};
use sel4_common::utils::convert_to_mut_type_ref;
#[cfg(not(feature = "KERNEL_MCS"))]
use sel4_common::utils::ptr_to_mut;
#[cfg(not(feature = "KERNEL_MCS"))]
use sel4_ipc::Transfer;
use sel4_ipc::{endpoint_func, notification_func};
#[cfg(not(feature = "KERNEL_MCS"))]
use sel4_task::rescheduleRequired;
use sel4_task::{
    activateThread, get_currenct_thread, schedule, set_thread_state, tcb_t, ThreadState,
};
#[cfg(feature = "KERNEL_MCS")]
use sel4_task::{chargeBudget, get_current_sc, ksConsumed, mcs_preemption_point};
pub use utils::*;

use crate::arch::restore_user_context;
use crate::interrupt::handler::handleInterrupt;
use crate::kernel::boot::current_lookup_fault;
use crate::{config::irqInvalid, interrupt::getActiveIRQ};
use sel4_common::ffi::current_fault;

use self::invocation::handleInvocation;

#[no_mangle]
pub fn slowpath(syscall: usize) {
    if (syscall as isize) < SYSCALL_MIN || (syscall as isize) > SYSCALL_MAX {
        // using ffi_call! macro to call c function
        handleUnknownSyscall(syscall as isize);
        // ffi_call!(handleUnknownSyscall(id: usize => syscall));
    } else {
        handleSyscall(syscall);
    }
    restore_user_context();
}

#[no_mangle]
#[cfg(not(feature = "KERNEL_MCS"))]
pub fn handleSyscall(_syscall: usize) -> exception_t {
    let syscall: isize = _syscall as isize;
    // if hart_id() == 0 {
    //     debug!("handle syscall: {}", syscall);
    // }
    // sel4_common::println!("handle syscall {}", syscall);
    match syscall {
        SysSend => {
            let ret = handleInvocation(false, true);
            if unlikely(ret != exception_t::EXCEPTION_NONE) {
                let irq = getActiveIRQ();
                if irq != irqInvalid {
                    handleInterrupt(irq);
                }
            }
        }
        SysNBSend => {
            let ret = handleInvocation(false, false);
            if unlikely(ret != exception_t::EXCEPTION_NONE) {
                let irq = getActiveIRQ();
                if irq != irqInvalid {
                    handleInterrupt(irq);
                }
            }
        }
        SysCall => {
            let ret = handleInvocation(true, true);
            if unlikely(ret != exception_t::EXCEPTION_NONE) {
                let irq = getActiveIRQ();
                if irq != irqInvalid {
                    handleInterrupt(irq);
                }
            }
        }
        SysRecv => {
            handle_recv(true);
        }
        SysReply => handle_reply(),
        SysReplyRecv => {
            handle_reply();
            handle_recv(true);
        }
        SysNBRecv => handle_recv(false),
        SysYield => handle_yield(),
        _ => panic!("Invalid syscall"),
    }
    schedule();
    activateThread();
    exception_t::EXCEPTION_NONE
}
#[no_mangle]
#[cfg(feature = "KERNEL_MCS")]
pub fn handleSyscall(_syscall: usize) -> exception_t {
    use core::intrinsics::likely;

    use sel4_task::{checkBudgetRestart, updateTimestamp};

    let syscall: isize = _syscall as isize;
    // if hart_id() == 0 {
    //     debug!("handle syscall: {}", syscall);
    // }
    // sel4_common::println!("handle syscall {}", syscall);
    updateTimestamp();
    if likely(checkBudgetRestart()) {
        match syscall {
            SysSend => {
                let ret = handleInvocation(
                    false,
                    true,
                    false,
                    false,
                    get_currenct_thread().tcbArch.get_register(Cap),
                );
                if unlikely(ret != exception_t::EXCEPTION_NONE) {
                    mcs_preemption_point();
                    let irq = getActiveIRQ();
                    if irq != irqInvalid {
                        handleInterrupt(irq);
                    }
                }
            }
            SysNBSend => {
                let ret = handleInvocation(
                    false,
                    false,
                    false,
                    false,
                    get_currenct_thread().tcbArch.get_register(Cap),
                );
                if unlikely(ret != exception_t::EXCEPTION_NONE) {
                    mcs_preemption_point();
                    let irq = getActiveIRQ();
                    if irq != irqInvalid {
                        handleInterrupt(irq);
                    }
                }
            }
            SysCall => {
                let ret = handleInvocation(
                    true,
                    true,
                    true,
                    false,
                    get_currenct_thread().tcbArch.get_register(Cap),
                );
                if unlikely(ret != exception_t::EXCEPTION_NONE) {
                    mcs_preemption_point();
                    let irq = getActiveIRQ();
                    if irq != irqInvalid {
                        handleInterrupt(irq);
                    }
                }
            }
            SysRecv => {
                handle_recv(true, true);
            }
            SysWait => {
                handle_recv(true, false);
            }
            SysNBWait => {
                handle_recv(false, false);
            }
            SysReplyRecv => {
                let reply = get_currenct_thread().tcbArch.get_register(Reply);
                let ret = handleInvocation(false, false, true, true, reply);
                assert!(ret == exception_t::EXCEPTION_NONE);
                handle_recv(true, true);
            }
            SysNBSendRecv => {
                // TODO: MCS
                let dest = get_currenct_thread().tcbArch.get_register(nbsRecvDest);
                let ret = handleInvocation(false, false, true, true, dest);
                if unlikely(ret != exception_t::EXCEPTION_NONE) {
                    mcs_preemption_point();
                    let irq = getActiveIRQ();
                    if irq != irqInvalid {
                        handleInterrupt(irq);
                    }
                } else {
                    handle_recv(true, true);
                }
            }
            SysNBSendWait => {
                let reply = get_currenct_thread().tcbArch.get_register(Reply);
                let ret = handleInvocation(false, false, true, true, reply);
                if unlikely(ret != exception_t::EXCEPTION_NONE) {
                    mcs_preemption_point();
                    let irq = getActiveIRQ();
                    if irq != irqInvalid {
                        handleInterrupt(irq);
                    }
                } else {
                    handle_recv(true, false);
                }
            }
            SysNBRecv => handle_recv(false, true),
            SysYield => handle_yield(),
            _ => panic!("Invalid syscall"),
        }
    }
    schedule();
    activateThread();
    exception_t::EXCEPTION_NONE
}
#[cfg(feature = "KERNEL_MCS")]
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
#[cfg(not(feature = "KERNEL_MCS"))]
fn send_fault_ipc(thread: &mut tcb_t) -> exception_t {
    use sel4_common::structures_gen::seL4_Fault_tag;

    let origin_lookup_fault = unsafe { current_lookup_fault.clone() };
    let lu_ret = thread.lookup_slot(thread.tcbFaultHandler);
    if lu_ret.status != exception_t::EXCEPTION_NONE {
        unsafe {
            current_fault = seL4_Fault_CapFault::new(thread.tcbFaultHandler as u64, 0).unsplay();
        }
        return exception_t::EXCEPTION_FAULT;
    }
    let handler_cap = cap::cap_endpoint_cap(&ptr_to_mut(lu_ret.slot).capability);
    if handler_cap.clone().unsplay().get_tag() == cap_tag::cap_endpoint_cap
        && (handler_cap.get_capCanGrant() != 0 || handler_cap.get_capCanGrantReply() != 0)
    {
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
            current_fault = seL4_Fault_CapFault::new(thread.tcbFaultHandler as u64, 0).unsplay();
            current_lookup_fault = lookup_fault_missing_capability::new(0).unsplay();
        }
        return exception_t::EXCEPTION_FAULT;
    }
    exception_t::EXCEPTION_NONE
}

#[inline]
#[cfg(not(feature = "KERNEL_MCS"))]
pub fn handle_fault(thread: &mut tcb_t) {
    if send_fault_ipc(thread) != exception_t::EXCEPTION_NONE {
        set_thread_state(thread, ThreadState::ThreadStateInactive);
    }
}
#[inline]
#[cfg(feature = "KERNEL_MCS")]
pub fn handle_fault(thread: &mut tcb_t) {
    use sel4_common::sel4_config::tcbFaultHandler;
    let cte = thread.get_cspace(tcbFaultHandler);
    let hasFaultHandler = send_fault_ipc(thread, &cte.capability, thread.tcbSchedContext != 0);
    if !hasFaultHandler {
        set_thread_state(thread, ThreadState::ThreadStateInactive);
    }
}
#[inline]
#[cfg(feature = "KERNEL_MCS")]
#[no_mangle]
pub fn handleTimeout(tptr: &mut tcb_t) {
    use sel4_common::sel4_config::tcbTimeoutHandler;

    assert!(tptr.validTimeoutHandler());
    let cte = tptr.get_cspace(tcbTimeoutHandler);
    send_fault_ipc(tptr, &cte.capability, false);
}
#[inline]
#[cfg(feature = "KERNEL_MCS")]
#[no_mangle]
pub fn endTimeslice(can_timeout_fault: bool) {
    use sel4_common::structures_gen::seL4_Fault_Timeout;

    unsafe {
        let thread = get_currenct_thread();
        let sched_context = get_current_sc();
        if can_timeout_fault && !sched_context.is_round_robin() && thread.validTimeoutHandler() {
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
#[cfg(feature = "KERNEL_MCS")]
#[inline]
pub fn lookupReply() -> lookupCap_ret_t {
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
#[cfg(not(feature = "KERNEL_MCS"))]
fn handle_reply() {
    let current_thread = get_currenct_thread();
    let caller_slot = current_thread.get_cspace_mut_ref(tcbCaller);
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
#[cfg(feature = "KERNEL_MCS")]
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
                let lu_ret = lookupReply();
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

#[cfg(not(feature = "KERNEL_MCS"))]
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
    #[cfg(feature = "KERNEL_MCS")]
    {
        unsafe {
            let consumed = get_current_sc().scConsumed + ksConsumed;
            chargeBudget((*get_current_sc().refill_head()).rAmount, false);
            get_current_sc().scConsumed = consumed;
        }
    }
    #[cfg(not(feature = "KERNEL_MCS"))]
    {
        get_currenct_thread().sched_dequeue();
        get_currenct_thread().sched_append();
        rescheduleRequired();
    }
}
