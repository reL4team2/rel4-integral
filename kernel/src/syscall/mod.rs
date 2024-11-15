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
pub const SysReplyRecv: isize = -2;
pub const SysSend: isize = -3;
pub const SysNBSend: isize = -4;
pub const SysRecv: isize = -5;
pub const SysReply: isize = -6;
pub const SysYield: isize = -7;
pub const SysNBRecv: isize = -8;

pub const SysDebugPutChar: isize = -9;
pub const SysDebugDumpScheduler: isize = -10;
pub const SysDebugHalt: isize = -11;
pub const SysDebugCapIdentify: isize = -12;
pub const SysDebugSnapshot: isize = -13;
pub const SysDebugNameThread: isize = -14;
pub const SysGetClock: isize = -30;
use sel4_common::structures::exception_t;
use sel4_common::structures_gen::{
    cap, cap_Splayed, cap_tag, endpoint, lookup_fault_missing_capability, notification,
    seL4_Fault_CapFault, seL4_Fault_tag,
};
use sel4_common::utils::{convert_to_mut_type_ref, ptr_to_mut};
use sel4_ipc::{endpoint_func, notification_func, Transfer};
use sel4_task::{
    activateThread, get_currenct_thread, rescheduleRequired, schedule, set_thread_state, tcb_t,
    ThreadState,
};
pub use utils::*;

use crate::arch::restore_user_context;
use crate::interrupt::handler::handleInterrupt;
use crate::kernel::boot::{current_fault, current_lookup_fault};
use crate::{config::irqInvalid, interrupt::getActiveIRQ};

use self::invocation::handleInvocation;

#[no_mangle]
pub fn slowpath(syscall: usize) {
    if (syscall as isize) < -8 || (syscall as isize) > -1 {
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
    let syscall: isize = _syscall as isize;
    // if hart_id() == 0 {
    //     debug!("handle syscall: {}", syscall);
    // }
    // TODO: MCS
    // updateTimestamp();
    // if (likely(checkBudgetRestart())) {
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
                let irq = getActiveIRQ();
                if irq != irqInvalid {
                    handleInterrupt(irq);
                }
            }
        }
        SysRecv => {
            // TODO: MCS
            handle_recv(true, true);
        }
        // TODO: MCS
        SysNBRecv => {
            // TODO: MCS
            handle_recv(true, true)
        }
        SysYield => handle_yield(),
        _ => panic!("Invalid syscall"),
    }
    // }
    schedule();
    activateThread();
    exception_t::EXCEPTION_NONE
}

fn send_fault_ipc(thread: &mut tcb_t) -> exception_t {
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
pub fn handle_fault(thread: &mut tcb_t) {
    if send_fault_ipc(thread) != exception_t::EXCEPTION_NONE {
        set_thread_state(thread, ThreadState::ThreadStateInactive);
    }
}
// #[cfg(feature="KERNEL_MCS")]
// #[inline]
// pub fn lookupReply() -> lookupCap_ret_t
// TODO: MCS
#[cfg(not(feature = "KERNEL_MCS"))]
fn handle_reply() {
    let current_thread = get_currenct_thread();
    let caller_slot = current_thread.get_cspace_mut_ref(tcbCaller);
    let caller_cap = cap::cap_reply_cap(&caller_slot.capability);
    if caller_cap.clone().unsplay().get_tag() == cap_tag::cap_reply_cap {
        if caller_cap.get_capReplyMaster() != 0 {
            return;
        }
        let caller = convert_to_mut_type_ref::<tcb_t>(caller_cap.get_capTCBPtr() as usize);
        current_thread.do_reply(caller, caller_slot, caller_cap.get_capReplyCanGrant() != 0);
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
        // TODO: MCS
    }
    #[cfg(not(feature = "KERNEL_MCS"))]
    {
        get_currenct_thread().sched_dequeue();
        get_currenct_thread().sched_append();
        rescheduleRequired();
    }
}
