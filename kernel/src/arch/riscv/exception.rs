use super::read_stval;
use crate::compatibility::lookupIPCBuffer;
use crate::config::*;
use crate::halt;
use crate::object::lookupCapAndSlot;
use crate::strnlen;
use crate::syscall::handle_fault;
use crate::syscall::{
    SysDebugCapIdentify, SysDebugDumpScheduler, SysDebugHalt, SysDebugNameThread, SysDebugPutChar,
    SysDebugSnapshot, SysGetClock,
};
#[cfg(feature = "KERNEL_MCS")]
use core::intrinsics::likely;
use log::debug;
use sel4_common::arch::ArchReg::*;
use sel4_common::ffi::current_fault;
use sel4_common::platform::read_time;
use sel4_common::print;
use sel4_common::sel4_config::seL4_MsgMaxLength;
use sel4_common::structures::exception_t;
use sel4_common::structures_gen::cap_tag;
use sel4_common::structures_gen::seL4_Fault_UnknownSyscall;
use sel4_common::structures_gen::seL4_Fault_UserException;
use sel4_common::structures_gen::seL4_Fault_VMFault;
use sel4_task::{activateThread, get_currenct_thread, schedule};
#[cfg(feature = "KERNEL_MCS")]
use sel4_task::{checkBudgetRestart, updateTimestamp};

#[no_mangle]
pub fn handleUnknownSyscall(w: isize) -> exception_t {
    let thread = get_currenct_thread();
    if w == SysDebugPutChar {
        print!("{}", thread.tcbArch.get_register(Cap) as u8 as char);
        return exception_t::EXCEPTION_NONE;
    }
    if w == SysDebugDumpScheduler {
        // unimplement debug
        // println!("debug dump scheduler");
        return exception_t::EXCEPTION_NONE;
    }
    if w == SysDebugHalt {
        // unimplement debug
        // println!("debug halt");
        return exception_t::EXCEPTION_NONE;
    }
    if w == SysDebugSnapshot {
        // unimplement debug
        // println!("debug snap shot");
        return exception_t::EXCEPTION_NONE;
    }
    if w == SysDebugCapIdentify {
        let cptr = thread.tcbArch.get_register(Cap);
        let lu_ret = lookupCapAndSlot(thread, cptr);
        let cap_type = lu_ret.capability.get_tag();
        thread.tcbArch.set_register(Cap, cap_type as usize);
        return exception_t::EXCEPTION_NONE;
    }
    if w == SysDebugNameThread {
        let cptr = thread.tcbArch.get_register(Cap);
        let lu_ret = lookupCapAndSlot(thread, cptr);
        let cap_type = lu_ret.capability.get_tag();

        if cap_type != cap_tag::cap_thread_cap {
            debug!("SysDebugNameThread: cap is not a TCB, halting");
            halt();
        }
        let name = lookupIPCBuffer(true, thread) + 1;
        if name == 0 {
            debug!("SysDebugNameThread: Failed to lookup IPC buffer, halting");
            halt();
        }

        let len = strnlen(name as *const u8, seL4_MsgMaxLength * 8);
        if len == seL4_MsgMaxLength * 8 {
            debug!("SysDebugNameThread: Name too long, halting");
            halt();
        }

        // setThreadName(TCB_PTR(cap_thread_cap_get_capTCBPtr(lu_ret.cap)), name);
        return exception_t::EXCEPTION_NONE;
    }
    if w == SysGetClock {
        let current = read_time();
        thread.tcbArch.set_register(Cap, current);
        return exception_t::EXCEPTION_NONE;
    }
    unsafe {
        current_fault = seL4_Fault_UnknownSyscall::new(w as u64).unsplay();
        handle_fault(get_currenct_thread());
    }
    schedule();
    activateThread();
    exception_t::EXCEPTION_NONE
}

#[no_mangle]
pub fn handleUserLevelFault(w_a: usize, w_b: usize) -> exception_t {
    #[cfg(feature = "KERNEL_MCS")]
    {
        updateTimestamp();
        if likely(checkBudgetRestart()) {
            unsafe {
                current_fault = seL4_Fault_UserException::new(w_a as u64, w_b as u64).unsplay();
                handle_fault(get_currenct_thread());
            }
        }
    }
    #[cfg(not(feature = "KERNEL_MCS"))]
    unsafe {
        current_fault = seL4_Fault_UserException::new(w_a as u64, w_b as u64).unsplay();
        handle_fault(get_currenct_thread());
    }
    schedule();
    activateThread();
    exception_t::EXCEPTION_NONE
}

#[no_mangle]
pub fn handleVMFaultEvent(vm_faultType: usize) -> exception_t {
    #[cfg(feature = "KERNEL_MCS")]
    {
        updateTimestamp();
        if likely(checkBudgetRestart()) {
            let status = handle_vm_fault(vm_faultType);
            if status != exception_t::EXCEPTION_NONE {
                handle_fault(get_currenct_thread());
            }
        }
    }
    #[cfg(not(feature = "KERNEL_MCS"))]
    {
        let status = handle_vm_fault(vm_faultType);
        if status != exception_t::EXCEPTION_NONE {
            handle_fault(get_currenct_thread());
        }
    }
    schedule();
    activateThread();
    exception_t::EXCEPTION_NONE
}

pub fn handle_vm_fault(type_: usize) -> exception_t {
    let addr = read_stval();
    match type_ {
        RISCVLoadPageFault | RISCVLoadAccessFault => {
            unsafe {
                current_fault =
                    seL4_Fault_VMFault::new(addr as u64, RISCVLoadAccessFault as u64, 0).unsplay();
            }
            exception_t::EXCEPTION_FAULT
        }
        RISCVStorePageFault | RISCVStoreAccessFault => {
            unsafe {
                current_fault =
                    seL4_Fault_VMFault::new(addr as u64, RISCVStoreAccessFault as u64, 0).unsplay();
            }
            exception_t::EXCEPTION_FAULT
        }
        RISCVInstructionAccessFault | RISCVInstructionPageFault => {
            unsafe {
                current_fault =
                    seL4_Fault_VMFault::new(addr as u64, RISCVInstructionAccessFault as u64, 1)
                        .unsplay();
            }
            exception_t::EXCEPTION_FAULT
        }
        _ => panic!("Invalid VM fault type:{}", type_),
    }
}
