#[cfg(feature = "KERNEL_MCS")]
use core::intrinsics::likely;

use crate::arch::aarch64::consts::ARMDataAbort;
use crate::arch::aarch64::consts::ARMPrefetchAbort;
use crate::compatibility::lookupIPCBuffer;
use crate::halt;
use crate::object::lookupCapAndSlot;
use crate::strnlen;
use crate::syscall::handle_fault;
use crate::syscall::{
    SysDebugCapIdentify, SysDebugDumpScheduler, SysDebugHalt, SysDebugNameThread, SysDebugPutChar,
    SysDebugSnapshot, SysGetClock,SysShutdown
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
#[cfg(feature = "KERNEL_MCS")]
use sel4_task::{checkBudgetRestart, updateTimestamp};

use super::instruction::*;

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
        /*no implementation of aarch64 get clock*/
        let current = timer.getCurrentTime();
        thread.tcbArch.set_register(Cap, current);
        return exception_t::EXCEPTION_NONE;
    }
	if w == SysShutdown{
		sel4_common::arch::shutdown();
	}
    #[cfg(not(feature = "KERNEL_MCS"))]
    unsafe {
        current_fault = seL4_Fault_UnknownSyscall::new(w as u64).unsplay();
        handle_fault(get_currenct_thread());
    }
    #[cfg(feature = "KERNEL_MCS")]
    {
        updateTimestamp();
        if likely(checkBudgetRestart()) {
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
        case ARMDataAbort: {
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
        case ARMPrefetchAbort: {
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
    // ARMDataAbort = seL4_DataFault,               0
    // ARMPrefetchAbort = seL4_InstructionFault     1
    log::debug!(
        "Handle VM fault: {}  domain: {}",
        type_,
        get_current_domain()
    );
    match type_ {
        ARMDataAbort => {
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
        ARMPrefetchAbort => {
            let pc = get_currenct_thread().tcbArch.get_register(FaultIP);
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
