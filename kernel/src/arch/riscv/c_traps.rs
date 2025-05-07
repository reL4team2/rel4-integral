use core::arch::asm;

use super::read_scause;
use crate::syscall::slowpath;

#[cfg(feature = "have_fpu")]
use crate::arch::fpu::{handle_fpu_fault, is_fpu_enable, lazy_fpu_restore, set_tcb_fs_state};
use sel4_common::arch::ArchReg;
use sel4_common::sel4_config::{
    RISCV_INSTRUCTION_ACCESS_FAULT, RISCV_INSTRUCTION_PAGE_FAULT, RISCV_LOAD_ACCESS_FAULT,
    RISCV_LOAD_PAGE_FAULT, RISCV_STORE_ACCESS_FAULT, RISCV_STORE_PAGE_FAULT,
};

use sel4_task::*;

use super::exception::{handleUserLevelFault, handleVMFaultEvent};
use crate::interrupt::handler::handle_interrupt_entry;

#[cfg(feature = "enable_smp")]
use crate::{
    ffi::{clh_is_self_in_queue, clh_lock_acquire, clh_lock_release},
    interrupt::get_active_irq,
};

#[cfg(feature = "enable_smp")]
use sel4_common::utils::cpu_id;

#[no_mangle]
pub fn restore_user_context() {
    unsafe {
        // debug!("restore_user_context");
        let cur_thread_reg: usize = get_currenct_thread().tcbArch.raw_ptr();
        #[cfg(feature = "enable_smp")]
        {
            if clh_is_self_in_queue() {
                clh_lock_release(cpu_id());
            }
            // debug!("restore_user_context2");
            let mut cur_sp: usize = 8;
            asm!(
                "csrr {}, sscratch",
                out(reg) cur_sp,
            );
            // debug!("cur_sp: {:#x}", cur_sp);
            *((cur_sp - 8) as *mut usize) = cur_thread_reg;
        }
        #[cfg(feature = "have_fpu")]
        {
            lazy_fpu_restore(get_currenct_thread());
            set_tcb_fs_state(get_currenct_thread(), is_fpu_enable());
        }
        // debug!("restore_user_context3");
        asm!("mv t0, {0}      \n",
        "ld  ra, (0*8)(t0)  \n",
        "ld  sp, (1*8)(t0)  \n",
        "ld  gp, (2*8)(t0)  \n",
        "ld  t2, (6*8)(t0)  \n",
        "ld  s0, (7*8)(t0)  \n",
        "ld  s1, (8*8)(t0)  \n",
        "ld  a0, (9*8)(t0)  \n",
        "ld  a1, (10*8)(t0) \n",
        "ld  a2, (11*8)(t0) \n",
        "ld  a3, (12*8)(t0) \n",
        "ld  a4, (13*8)(t0) \n",
        "ld  a5, (14*8)(t0) \n",
        "ld  a6, (15*8)(t0) \n",
        "ld  a7, (16*8)(t0) \n",
        "ld  s2, (17*8)(t0) \n",
        "ld  s3, (18*8)(t0) \n",
        "ld  s4, (19*8)(t0) \n",
        "ld  s5, (20*8)(t0) \n",
        "ld  s6, (21*8)(t0) \n",
        "ld  s7, (22*8)(t0) \n",
        "ld  s8, (23*8)(t0) \n",
        "ld  s9, (24*8)(t0) \n",
        "ld  s10, (25*8)(t0)\n",
        "ld  s11, (26*8)(t0)\n",
        "ld  t3, (27*8)(t0) \n",
        "ld  t4, (28*8)(t0) \n",
        "ld  t5, (29*8)(t0) \n",
        "ld  t6, (30*8)(t0) \n",
        "ld  t1, (3*8)(t0)  \n",
        "add tp, t1, x0  \n",
        "ld  t1, (34*8)(t0)\n",
        "csrw sepc, t1", in(reg) cur_thread_reg);

        #[cfg(not(feature = "enable_smp"))]
        {
            asm!("csrw sscratch, t0")
        }
        asm!(
            "ld  t1, (32*8)(t0) \n",
            "csrw sstatus, t1\n",
            "ld  t1, (5*8)(t0) \n",
            "ld  t0, (4*8)(t0) \n",
            "sret"
        );
        panic!("unreachable")
    }
}

#[inline]
#[no_mangle]
pub fn fastpath_restore(_badge: usize, _msgInfo: usize, cur_thread: *mut tcb_t) {
    unsafe {
        let cur_thread_reg = (*cur_thread).tcbArch.raw_ptr() as usize;
        #[cfg(feature = "enable_smp")]
        {
            if clh_is_self_in_queue() {
                clh_lock_release(cpu_id());
            }
            use core::arch::asm;
            let mut sp: usize;
            asm!(
                "csrr {0}, sscratch",
                out(reg) sp,
            );
            sp -= core::mem::size_of::<usize>();
            let ptr = sp as *mut usize;
            *ptr = (*cur_thread).tcbArch.raw_ptr();
        }
        #[cfg(feature = "have_fpu")]
        {
            lazy_fpu_restore(get_currenct_thread());
            set_tcb_fs_state(get_currenct_thread(), is_fpu_enable());
        }

        asm!("mv a0, {0}      \n",
        "mv  a1, {1} \n",
        "mv  t0, {2} \n",
        "ld  ra, (0*8)(t0)  \n",
        "ld  sp, (1*8)(t0)  \n",
        "ld  gp, (2*8)(t0)  \n",
        "ld  t2, (6*8)(t0)  \n",
        "ld  s0, (7*8)(t0)  \n",
        "ld  s1, (8*8)(t0)  \n",
        "ld  a2, (11*8)(t0) \n",
        "ld  a3, (12*8)(t0) \n",
        "ld  a4, (13*8)(t0) \n",
        "ld  a5, (14*8)(t0) \n",
        "ld  a6, (15*8)(t0) \n",
        "ld  a7, (16*8)(t0) \n",
        "ld  s2, (17*8)(t0) \n",
        "ld  s3, (18*8)(t0) \n",
        "ld  s4, (19*8)(t0) \n",
        "ld  s5, (20*8)(t0) \n",
        "ld  s6, (21*8)(t0) \n",
        "ld  s7, (22*8)(t0) \n",
        "ld  s8, (23*8)(t0) \n",
        "ld  s9, (24*8)(t0) \n",
        "ld  s10, (25*8)(t0)\n",
        "ld  s11, (26*8)(t0)\n",
        "ld  t3, (27*8)(t0) \n",
        "ld  t4, (28*8)(t0) \n",
        "ld  t5, (29*8)(t0) \n",
        "ld  t6, (30*8)(t0) \n",
        "ld  t1, (3*8)(t0)  \n",
        "add tp, t1, x0  \n",
        "ld  t1, (34*8)(t0)\n",
        "csrw sepc, t1",
        in(reg) _badge,
        in(reg) _msgInfo,
        in(reg) cur_thread_reg);

        #[cfg(not(feature = "enable_smp"))]
        {
            asm!("csrw sscratch, t0")
        }
        asm!(
            "ld  t1, (32*8)(t0) \n",
            "csrw sstatus, t1\n",
            "ld  t1, (5*8)(t0) \n",
            "ld  t0, (4*8)(t0) \n",
            "sret"
        );
    }
    panic!("unreachable")
}

#[no_mangle]
pub fn c_handle_interrupt() {
    // debug!("c_handle_interrupt");
    // if hart_id() != 0 {
    //     debug!("c_handle_interrupt");
    // }
    #[cfg(feature = "enable_smp")]
    {
        use sel4_common::platform::INTERRUPT_IPI_0;
        if get_active_irq() != INTERRUPT_IPI_0 {
            unsafe {
                clh_lock_acquire(cpu_id(), true);
            }
        }
    }
    // debug!("c_handle_interrupt");
    handle_interrupt_entry();
    restore_user_context();
}

#[no_mangle]
pub fn c_handle_exception() {
    #[cfg(feature = "enable_smp")]
    unsafe {
        clh_lock_acquire(cpu_id(), false);
    }
    // if hart_id() == 0 {
    //     debug!("c_handle_exception");
    // }

    let cause = read_scause();
    match cause {
        RISCV_INSTRUCTION_ACCESS_FAULT
        | RISCV_LOAD_ACCESS_FAULT
        | RISCV_STORE_ACCESS_FAULT
        | RISCV_LOAD_PAGE_FAULT
        | RISCV_STORE_PAGE_FAULT
        | RISCV_INSTRUCTION_PAGE_FAULT => {
            handleVMFaultEvent(cause);
        }
        _ => {
            // #ifdef CONFIG_HAVE_FPU
            //         if (!is_fpu_enable()) {
            //             /* we assume the illegal instruction is caused by FPU first */
            //             handle_fpu_fault();
            //             setNextPC(NODE_STATE(ksCurThread), getRestartPC(NODE_STATE(ksCurThread)));
            //             break;
            //         }
            // #endif
            unsafe {
                if !is_fpu_enable() {
                    handle_fpu_fault();
                    let pc = get_currenct_thread()
                        .tcbArch
                        .get_register(ArchReg::FAULT_IP);
                    get_currenct_thread()
                        .tcbArch
                        .set_register(ArchReg::NEXT_IP, pc);
                } else {
                    handleUserLevelFault(cause, 0);
                }
            }
        }
    }
    restore_user_context();
}

#[no_mangle]
pub fn c_handle_syscall(_cptr: usize, _msgInfo: usize, syscall: usize) {
    #[cfg(feature = "enable_smp")]
    unsafe {
        clh_lock_acquire(cpu_id(), false);
    }
    // if hart_id() == 0 {
    //     debug!("c_handle_syscall: syscall: {},", syscall as isize);
    // }
    slowpath(syscall);
    // debug!("c_handle_syscall complete");
}

#[no_mangle]
#[cfg(feature = "build_binary")]
#[link_section = ".text"]
pub fn c_handle_fastpath_call(cptr: usize, msgInfo: usize) {
    use crate::kernel::fastpath::fastpath_call;
    fastpath_call(cptr, msgInfo);
}

#[no_mangle]
#[cfg(feature = "build_binary")]
#[link_section = ".text"]
#[cfg(not(feature = "kernel_mcs"))]
pub fn c_handle_fastpath_reply_recv(cptr: usize, msgInfo: usize) {
    use crate::kernel::fastpath::fastpath_reply_recv;
    fastpath_reply_recv(cptr, msgInfo);
}

#[no_mangle]
#[cfg(feature = "build_binary")]
#[link_section = ".text"]
#[cfg(feature = "kernel_mcs")]
pub fn c_handle_fastpath_reply_recv(cptr: usize, msgInfo: usize, reply: usize) {
    use crate::kernel::fastpath::fastpath_reply_recv;
    fastpath_reply_recv(cptr, msgInfo, reply);
}
