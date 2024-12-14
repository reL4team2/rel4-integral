pub(crate) mod arch;
pub mod decode;
mod invoke_cnode;
pub mod invoke_irq;
mod invoke_mmu_op;
#[cfg(feature = "KERNEL_MCS")]
mod invoke_sched;
mod invoke_tcb;
mod invoke_untyped;

use core::intrinsics::unlikely;

use log::debug;
use sel4_common::arch::{msgRegisterNum, ArchReg};
use sel4_common::message_info::seL4_MessageInfo_func;
use sel4_common::shared_types_bf_gen::seL4_MessageInfo;
use sel4_common::structures::exception_t;
use sel4_common::structures_gen::seL4_Fault_CapFault;
use sel4_task::{get_currenct_thread, set_thread_state, ThreadState};

use crate::syscall::invocation::decode::decode_invocation;
use crate::syscall::syscall_reply::{reply_error_from_kernel, reply_success_from_kernel};
use crate::syscall::{handle_fault, lookup_extra_caps_with_buf};
use sel4_common::ffi::current_fault;

#[no_mangle]
#[cfg(not(feature = "KERNEL_MCS"))]
pub fn handleInvocation(isCall: bool, isBlocking: bool) -> exception_t {
    let thread = get_currenct_thread();
    let info = seL4_MessageInfo::from_word_security(thread.tcbArch.get_register(ArchReg::MsgInfo));
    let cptr = thread.tcbArch.get_register(ArchReg::Cap);
    let lu_ret = thread.lookup_slot(cptr);
    if unlikely(lu_ret.status != exception_t::EXCEPTION_NONE) {
        debug!("Invocation of invalid cap {:#x}.", cptr);
        unsafe {
            current_fault = seL4_Fault_CapFault::new(cptr as u64, 0).unsplay();
        }
        if isBlocking {
            handle_fault(thread);
        }
        return exception_t::EXCEPTION_NONE;
    }
    let buffer = thread.lookup_ipc_buffer(false);
    let status = lookup_extra_caps_with_buf(thread, buffer);
    if unlikely(status != exception_t::EXCEPTION_NONE) {
        debug!("Lookup of extra caps failed.");
        if isBlocking {
            // handleFault(thread);
            handle_fault(thread);
        }
        return exception_t::EXCEPTION_NONE;
    }

    let mut length = info.get_length() as usize;
    if unlikely(length > msgRegisterNum && buffer.is_none()) {
        length = msgRegisterNum;
    }

    let capability = unsafe { (*(lu_ret.slot)).capability.clone() };
    let status = decode_invocation(
        info.get_message_label(),
        length,
        unsafe { &mut *lu_ret.slot },
        &capability,
        cptr,
        isBlocking,
        isCall,
        buffer.unwrap(),
    );
    if status == exception_t::EXCEPTION_PREEMTED {
        return status;
    }

    if status == exception_t::EXCEPTION_SYSCALL_ERROR {
        if isCall {
            reply_error_from_kernel(thread);
        }
        return exception_t::EXCEPTION_NONE;
    }

    if unlikely(thread.get_state() == ThreadState::ThreadStateRestart) {
        if isCall {
            reply_success_from_kernel(thread);
        }
        set_thread_state(thread, ThreadState::ThreadStateRunning);
    }
    return exception_t::EXCEPTION_NONE;
}
#[no_mangle]
#[cfg(feature = "KERNEL_MCS")]
// TODO: MCS
pub fn handleInvocation(
    isCall: bool,
    isBlocking: bool,
    canDonate: bool,
    firstPhase: bool,
    cptr: usize,
) -> exception_t {
    let thread = get_currenct_thread();
    let info = seL4_MessageInfo::from_word_security(thread.tcbArch.get_register(ArchReg::MsgInfo));
    let lu_ret = thread.lookup_slot(cptr);
    if unlikely(lu_ret.status != exception_t::EXCEPTION_NONE) {
        debug!("Invocation of invalid cap {:#x}.", cptr);
        unsafe {
            current_fault = seL4_Fault_CapFault::new(cptr as u64, 0).unsplay();
        }
        if isBlocking {
            handle_fault(thread);
        }
        return exception_t::EXCEPTION_NONE;
    }
    let buffer = thread.lookup_ipc_buffer(false);
    let status = lookup_extra_caps_with_buf(thread, buffer);
    if unlikely(status != exception_t::EXCEPTION_NONE) {
        debug!("Lookup of extra caps failed.");
        if isBlocking {
            // handleFault(thread);
            handle_fault(thread);
        }
        return exception_t::EXCEPTION_NONE;
    }

    let mut length = info.get_length() as usize;
    if unlikely(length > msgRegisterNum && buffer.is_none()) {
        length = msgRegisterNum;
    }

    let capability = unsafe { (*(lu_ret.slot)).capability.clone() };
    // #ifdef CONFIG_KERNEL_MCS
    //     status = decodeInvocation(seL4_MessageInfo_get_label(info), length,
    //                               cptr, lu_ret.slot, lu_ret.cap,
    //                               isBlocking, isCall,
    //                               canDonate, firstPhase, buffer);
    let status = decode_invocation(
        info.get_message_label(),
        length,
        unsafe { &mut *lu_ret.slot },
        &capability,
        cptr,
        isBlocking,
        isCall,
        canDonate,
        firstPhase,
        buffer.unwrap(),
    );
    if status == exception_t::EXCEPTION_PREEMTED {
        return status;
    }

    if status == exception_t::EXCEPTION_SYSCALL_ERROR {
        if isCall {
            reply_error_from_kernel(thread);
        }
        return exception_t::EXCEPTION_NONE;
    }

    if unlikely(thread.get_state() == ThreadState::ThreadStateRestart) {
        if isCall {
            reply_success_from_kernel(thread);
        }
        set_thread_state(thread, ThreadState::ThreadStateRunning);
    }
    return exception_t::EXCEPTION_NONE;
}
