use super::read_stval;
use crate::{
    compatibility::lookup_ipc_buffer,
    halt,
    object::lookupCapAndSlot,
    strnlen,
    syscall::{
        handle_fault, SYS_DEBUG_CAP_IDENTIFY, SYS_DEBUG_DUMP_SCHEDULER, SYS_DEBUG_HALT,
        SYS_DEBUG_NAME_THREAD, SYS_DEBUG_PUT_CHAR, SYS_DEBUG_SNAPSHOT, SYS_GET_CLOCK,
    },
};
#[cfg(feature = "kernel_mcs")]
use core::intrinsics::likely;
use log::debug;
use sel4_common::{
    arch::ArchReg::*,
    ffi::current_fault,
    platform::read_time,
    print,
    sel4_config::*,
    structures::exception_t,
    structures_gen::{
        cap_tag, seL4_Fault_UnknownSyscall, seL4_Fault_UserException, seL4_Fault_VMFault,
    },
};
use sel4_task::{activateThread, get_currenct_thread, schedule};
#[cfg(feature = "kernel_mcs")]
use sel4_task::{check_budget_restart, update_timestamp};

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
        // println!("debug snap shot");
        return exception_t::EXCEPTION_NONE;
    }
    if w == SYS_DEBUG_CAP_IDENTIFY {
        let cptr = thread.tcbArch.get_register(Cap);
        let lu_ret = lookupCapAndSlot(thread, cptr);
        let cap_type = lu_ret.capability.get_tag();
        thread.tcbArch.set_register(Cap, cap_type as usize);
        return exception_t::EXCEPTION_NONE;
    }
    if w == SYS_DEBUG_NAME_THREAD {
        let cptr = thread.tcbArch.get_register(Cap);
        let lu_ret = lookupCapAndSlot(thread, cptr);
        let cap_type = lu_ret.capability.get_tag();

        if cap_type != cap_tag::cap_thread_cap {
            debug!("SYS_DEBUG_NAME_THREAD: cap is not a TCB, halting");
            halt();
        }
        let name = lookup_ipc_buffer(true, thread) + 1;
        if name == 0 {
            debug!("SYS_DEBUG_NAME_THREAD: Failed to lookup IPC buffer, halting");
            halt();
        }

        let len = strnlen(name as *const u8, SEL4_MSG_MAX_LENGTH * 8);
        if len == SEL4_MSG_MAX_LENGTH * 8 {
            debug!("SYS_DEBUG_NAME_THREAD: Name too long, halting");
            halt();
        }

        // setThreadName(TCB_PTR(cap_thread_cap_get_capTCBPtr(lu_ret.cap)), name);
        return exception_t::EXCEPTION_NONE;
    }
    if w == SYS_GET_CLOCK {
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
    schedule();
    activateThread();
    exception_t::EXCEPTION_NONE
}

pub fn handle_vm_fault(type_: usize) -> exception_t {
    let addr = read_stval();
    match type_ {
        RISCV_LOAD_PAGE_FAULT | RISCV_LOAD_ACCESS_FAULT => {
            unsafe {
                current_fault =
                    seL4_Fault_VMFault::new(addr as u64, RISCV_LOAD_ACCESS_FAULT as u64, 0)
                        .unsplay();
            }
            exception_t::EXCEPTION_FAULT
        }
        RISCV_STORE_PAGE_FAULT | RISCV_STORE_ACCESS_FAULT => {
            unsafe {
                current_fault =
                    seL4_Fault_VMFault::new(addr as u64, RISCV_STORE_ACCESS_FAULT as u64, 0)
                        .unsplay();
            }
            exception_t::EXCEPTION_FAULT
        }
        RISCV_INSTRUCTION_ACCESS_FAULT | RISCV_INSTRUCTION_PAGE_FAULT => {
            unsafe {
                current_fault =
                    seL4_Fault_VMFault::new(addr as u64, RISCV_INSTRUCTION_ACCESS_FAULT as u64, 1)
                        .unsplay();
            }
            exception_t::EXCEPTION_FAULT
        }
        _ => panic!("Invalid VM fault type:{}", type_),
    }
}
