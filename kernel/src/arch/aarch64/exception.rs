#[cfg(feature = "kernel_mcs")]
use core::intrinsics::likely;

#[cfg(feature = "build_binary")]
use crate::arch::aarch64::c_traps::entry_hook;
use crate::arch::aarch64::consts::*;
use crate::compatibility::lookup_ipc_buffer;
use crate::halt;
use crate::object::lookupCapAndSlot;
use crate::strnlen;
use crate::syscall::handle_fault;
use crate::syscall::{
    SysDebugCapIdentify, SysDebugDumpScheduler, SysDebugHalt, SysDebugNameThread, SysDebugPutChar,
    SysDebugSnapshot, SysGetClock,
};

use aarch64_cpu::registers::Readable;
use aarch64_cpu::registers::TTBR0_EL1;
use log::debug;
use sel4_common::arch::ArchReg::*;
use sel4_common::ffi::current_fault;
use sel4_common::platform::timer;
use sel4_common::platform::Timer_func;
use sel4_common::print;
use sel4_common::sel4_config::seL4_MsgMaxLength;
use sel4_common::structures::exception_t;
use sel4_common::structures_gen::cap_tag;
use sel4_common::structures_gen::seL4_Fault_UnknownSyscall;
use sel4_common::structures_gen::seL4_Fault_UserException;
use sel4_common::structures_gen::seL4_Fault_VMFault;
use sel4_common::utils::global_read;
use sel4_task::{activateThread, get_currenct_thread, get_current_domain, schedule};
#[cfg(feature = "kernel_mcs")]
use sel4_task::{check_budget_restart, update_timestamp};

use super::instruction::*;
#[cfg(feature = "build_binary")]
use super::restore_user_context;

#[no_mangle]
pub fn handle_unknown_syscall(w: isize) -> exception_t {
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
        // println!("debug snapshot");
        return exception_t::EXCEPTION_NONE;
    }
    if w == SysDebugCapIdentify {
        // println!("debug cap identify");
        let cptr = thread.tcbArch.get_register(Cap);
        let lu_ret = lookupCapAndSlot(thread, cptr);
        let cap_type = lu_ret.capability.get_tag();
        thread.tcbArch.set_register(Cap, cap_type as usize);
        return exception_t::EXCEPTION_NONE;
    }
    if w == SysDebugNameThread {
        // println!("debug name thread");
        let cptr = thread.tcbArch.get_register(Cap);
        let lu_ret = lookupCapAndSlot(thread, cptr);
        let cap_type = lu_ret.capability.get_tag();

        if cap_type != cap_tag::cap_thread_cap {
            debug!("SysDebugNameThread: cap is not a TCB, halting");
            halt();
        }
        let name = lookup_ipc_buffer(true, thread) + 1;
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
        /*no implementation of aarch64 get clock*/
        let current = timer.get_current_time();
        thread.tcbArch.set_register(Cap, current);
        return exception_t::EXCEPTION_NONE;
    }
    #[cfg(not(feature = "kernel_mcs"))]
    unsafe {
        current_fault = seL4_Fault_UnknownSyscall::new(w as u64).unsplay();
        handle_fault(get_currenct_thread());
    }
    #[cfg(feature = "kernel_mcs")]
    {
        update_timestamp();
        if likely(check_budget_restart()) {
            unsafe {
                current_fault = seL4_Fault_UnknownSyscall::new(w as u64).unsplay();
                handle_fault(get_currenct_thread());
            }
        }
    }
    schedule();
    activateThread();
    exception_t::EXCEPTION_NONE
}

#[no_mangle]
pub fn handleUserLevelFault(w_a: usize, w_b: usize) -> exception_t {
    #[cfg(feature = "kernel_mcs")]
    {
        update_timestamp();
        if likely(check_budget_restart()) {
            unsafe {
                current_fault = seL4_Fault_UserException::new(w_a as u64, w_b as u64).unsplay();
                handle_fault(get_currenct_thread());
            }
        }
    }
    #[cfg(not(feature = "kernel_mcs"))]
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
    #[cfg(feature = "kernel_mcs")]
    {
        update_timestamp();
        if likely(check_budget_restart()) {
            let status = handle_vm_fault(vm_faultType);
            if status != exception_t::EXCEPTION_NONE {
                handle_fault(get_currenct_thread());
            }
        }
    }
    #[cfg(not(feature = "kernel_mcs"))]
    {
        let status = handle_vm_fault(vm_faultType);
        if status != exception_t::EXCEPTION_NONE {
            handle_fault(get_currenct_thread());
        }
    }

    // sel4_common::println!("handle vm fault event");
    schedule();
    activateThread();
    exception_t::EXCEPTION_NONE
}

pub fn handle_vm_fault(type_: usize) -> exception_t {
    /*
    exception_t handleVMFault(tcb_t *thread, vm_fault_type_t vm_faultType)
    {
        switch (vm_faultType) {
        case ARM_DATA_ABORT: {
            word_t addr, fault;
            addr = getFAR();
            fault = getDFSR();
    #ifdef CONFIG_ARM_HYPERVISOR_SUPPORT
            /* use the IPA */
            if (ARCH_NODE_STATE(armHSVCPUActive)) {
                addr = GET_PAR_ADDR(addressTranslateS1(addr)) | (addr & MASK(PAGE_BITS));
            }
    #endif
            current_fault = seL4_Fault_VMFault_new(addr, fault, false);
            return EXCEPTION_FAULT;
        }
        case ARM_PREFETCH_ABORT: {
            word_t pc, fault;
            pc = getRestartPC(thread);
            fault = getIFSR();

            current_fault = seL4_Fault_VMFault_new(pc, fault, true);
            return EXCEPTION_FAULT;
        }
        default:
            fail("Invalid VM fault type");
        }
    }
    */
    // ARM_DATA_ABORT = DATA_FAULT,               0
    // ARM_PREFETCH_ABORT = INSTRUCTION_FAULT     1
    log::debug!(
        "Handle VM fault: {}  domain: {}",
        type_,
        get_current_domain()
    );
    match type_ {
        ARM_DATA_ABORT => {
            let addr = get_far();
            let fault = get_esr();
            log::debug!("fault addr: {:#x} esr: {:#x}", addr, fault);
            unsafe {
                current_fault = seL4_Fault_VMFault::new(addr as u64, fault as u64, 0)
                    .unsplay()
                    .clone();
            }
            let current_fault_cpy = unsafe { current_fault.clone() };
            log::debug!("current_fault: {:#x?}", global_read!(current_fault_cpy));
            exception_t::EXCEPTION_FAULT
        }
        ARM_PREFETCH_ABORT => {
            let pc = get_currenct_thread().tcbArch.get_register(FAULT_IP);
            let fault = get_esr();
            unsafe {
                current_fault = seL4_Fault_VMFault::new(pc as u64, fault as u64, 1).unsplay();
            }

            log::debug!("ttbr0_el1: {:#x?}", TTBR0_EL1.get());

            log::debug!("fault pc: {:#x}  fault: {:#x}", pc, fault);
            exception_t::EXCEPTION_FAULT
        }
        _ => panic!("Invalid VM fault type:{}", type_),
    }
}

#[inline(always)]
#[cfg(feature = "build_binary")]
pub fn c_handle_vm_fault(type_: usize) -> ! {
    // TODO: NODE_LOCK needed in smp mod, now not supported
    entry_hook();
    handleVMFaultEvent(type_);
    restore_user_context();
    unreachable!()
}

#[no_mangle]
#[cfg(feature = "build_binary")]
pub fn c_handle_data_fault() -> ! {
    c_handle_vm_fault(DATA_FAULT)
}

#[no_mangle]
#[cfg(feature = "build_binary")]
pub fn c_handle_instruction_fault() -> ! {
    c_handle_vm_fault(INSTRUCTION_FAULT)
}
