#[cfg(feature = "kernel_mcs")]
use core::intrinsics::likely;

use aarch64_cpu::registers::Readable;
use log::error;
#[cfg(all(feature = "enable_smp", feature = "build_binary"))]
use sel4_common::utils::cpu_id;
use sel4_common::{
    arch::ArchReg::{self, *},
    ffi::current_fault,
    platform::{timer, Timer_func},
    print,
    sel4_config::{SEL4_MSG_MAX_LENGTH, SEL4_PAGE_BITS},
    structures::exception_t,
    structures_gen::{
        cap_tag, seL4_Fault_UnknownSyscall, seL4_Fault_UserException, seL4_Fault_VMFault,
    },
};
use sel4_task::{activateThread, get_currenct_thread, get_current_domain, schedule, ThreadState};
#[cfg(feature = "kernel_mcs")]
use sel4_task::{check_budget_restart, update_timestamp};
use sel4_vspace::PTE;

use super::instruction::*;
#[cfg(feature = "build_binary")]
use super::restore_user_context;
#[cfg(feature = "build_binary")]
use crate::arch::aarch64::c_traps::entry_hook;
#[cfg(all(feature = "enable_smp", feature = "build_binary"))]
use crate::smp::clh_lock_acquire;
use crate::{
    arch::aarch64::consts::*,
    compatibility::lookup_ipc_buffer,
    halt,
    object::lookupCapAndSlot,
    strnlen,
    syscall::{
        handle_fault, SYS_DEBUG_CAP_IDENTIFY, SYS_DEBUG_DUMP_SCHEDULER, SYS_DEBUG_HALT,
        SYS_DEBUG_NAME_THREAD, SYS_DEBUG_PUT_CHAR, SYS_DEBUG_SNAPSHOT, SYS_GET_CLOCK,
    },
};

#[no_mangle]
pub fn handle_unknown_syscall(w: isize) -> exception_t {
    let thread = get_currenct_thread();
    if w == SYS_DEBUG_PUT_CHAR {
        print!("{}", thread.tcbArch.get_register(Cap) as u8 as char);
        return exception_t::EXCEPTION_NONE;
    }
    if w == SYS_DEBUG_DUMP_SCHEDULER {
        // unimplement debug
        // println!("debug dump scheduler");
        return exception_t::EXCEPTION_NONE;
    }
    if w == SYS_DEBUG_HALT {
        // unimplement debug
        // println!("debug halt");
        return exception_t::EXCEPTION_NONE;
    }
    if w == SYS_DEBUG_SNAPSHOT {
        // unimplement debug
        // println!("debug snapshot");
        return exception_t::EXCEPTION_NONE;
    }
    if w == SYS_DEBUG_CAP_IDENTIFY {
        // println!("debug cap identify");
        let cptr = thread.tcbArch.get_register(Cap);
        let lu_ret = lookupCapAndSlot(thread, cptr);
        let cap_type = lu_ret.capability.get_tag();
        thread.tcbArch.set_register(Cap, cap_type as usize);
        return exception_t::EXCEPTION_NONE;
    }
    if w == SYS_DEBUG_NAME_THREAD {
        // println!("debug name thread");
        let cptr = thread.tcbArch.get_register(Cap);
        let lu_ret = lookupCapAndSlot(thread, cptr);
        let cap_type = lu_ret.capability.get_tag();

        if cap_type != cap_tag::cap_thread_cap {
            error!("SYS_DEBUG_NAME_THREAD: cap is not a TCB, halting");
            halt();
        }
        let name = lookup_ipc_buffer(true, thread) + 1;
        if name == 0 {
            error!("SYS_DEBUG_NAME_THREAD: Failed to lookup IPC buffer, halting");
            halt();
        }

        let len = strnlen(name as *const u8, SEL4_MSG_MAX_LENGTH * 8);
        if len == SEL4_MSG_MAX_LENGTH * 8 {
            error!("SYS_DEBUG_NAME_THREAD: Name too long, halting");
            halt();
        }

        // setThreadName(TCB_PTR(cap_thread_cap_get_capTCBPtr(lu_ret.cap)), name);
        return exception_t::EXCEPTION_NONE;
    }
    if w == SYS_GET_CLOCK {
        /* no implementation of aarch64 get clock */
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
    log::debug!("Handle VM fault: {}  domain: {}", type_, get_current_domain());
    match type_ {
        ARM_DATA_ABORT => {
            let mut addr = get_far();
            let fault = get_esr();
            #[cfg(feature = "hypervisor")]
            {
                // If VCPU is active, translate IPA to PA via AT S1E1R
                if crate::arch::vcpu::is_vcpu_active() {
                    let par = sel4_vspace::address_translate_s1(addr);
                    addr = sel4_vspace::get_par_addr(par) | (addr & mask_bits!(SEL4_PAGE_BITS));
                }
            }
            log::debug!("fault addr: {:#x} esr: {:#x}", addr, fault);
            unsafe {
                current_fault = seL4_Fault_VMFault::new(addr as u64, fault as u64, 0)
                    .unsplay()
                    .clone();
            }
            exception_t::EXCEPTION_FAULT
        },
        ARM_PREFETCH_ABORT => {
            let mut pc = get_currenct_thread().tcbArch.get_register(ArchReg::FaultIP);
            let fault = get_esr();
            #[cfg(feature = "hypervisor")]
            {
                // If VCPU is active, translate IPA to PA via AT S1E1R
                if crate::arch::vcpu::is_vcpu_active() {
                    let par = sel4_vspace::address_translate_s1(pc);
                    pc = sel4_vspace::get_par_addr(par) | (pc & mask_bits!(SEL4_PAGE_BITS));
                }
            }
            unsafe {
                current_fault = seL4_Fault_VMFault::new(pc as u64, fault as u64, 1).unsplay();
            }
            exception_t::EXCEPTION_FAULT
        },
        _ => panic!("Invalid VM fault type:{}", type_),
    }
}

#[inline(always)]
#[cfg(feature = "build_binary")]
pub fn c_handle_vm_fault(type_: usize) -> ! {
    #[cfg(feature = "enable_smp")]
    clh_lock_acquire(cpu_id(), false);
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

#[no_mangle]
pub fn c_handle_vcpu_fault(hsr: usize) -> ! {
    #[cfg(feature = "hypervisor")]
    {
        let handled = unsafe { super::vcpu::handle_vcpu_fault(hsr) };
        if !handled {
            schedule();
            activateThread();
            // Monitor: if the scheduler fell back to the idle thread, the VMM
            // (VCPU fault handler) wasn't runnable, so fault delivery failed.
            if get_currenct_thread().get_state() == ThreadState::ThreadStateIdleThreadState {
                error!("[VCPU] c_handle_vcpu_fault: switched to idle thread (VMM not schedulable?)");
            }
        }
    }
    #[cfg(feature = "build_binary")]
    {
        restore_user_context();
        unreachable!();
    }
    #[cfg(not(feature = "build_binary"))]
    loop {}
}
