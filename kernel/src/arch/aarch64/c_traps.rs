use crate::interrupt::handler::handle_interrupt_entry;
use crate::syscall::slow_path;
use core::arch::asm;

#[cfg(feature = "enable_smp")]
use crate::{
    ffi::{clh_is_self_in_queue, clh_lock_acquire, clh_lock_release},
    interrupt::get_active_irq,
};

#[cfg(feature = "enable_smp")]
use sel4_common::utils::cpu_id;
use sel4_task::*;

#[cfg(feature = "have_fpu")]
use crate::arch::fpu::lazy_fpu_restore;

#[no_mangle]
pub fn restore_user_context() {
    // NODE_UNLOCK_IF_HELD;

    // this is just a empty "do {} while (0)", I think it is only meaningfully under multi core case
    // at that case the micro NODE_UNLOCK_IF_HELD is
    // do {                         \
    //     if(clh_is_self_in_queue()) {                         \
    //         NODE_UNLOCK;                                     \
    //     }                                                    \
    // } while(0)

    // c_exit_hook();
    get_currenct_thread().tcbArch.load_thread_local();

    // TODO: I have already implement lazy_fpu_restore, But I am not very clearly about the fpu operator
    // So I project to add it in the next pull request
    // #ifdef CONFIG_HAVE_FPU
    //     lazy_fpu_restore(NODE_STATE(ksCurThread));
    // #endif /* CONFIG_HAVE_FPU */
    unsafe {
        #[cfg(feature = "enable_smp")]
        {
            if clh_is_self_in_queue() {
                clh_lock_release(cpu_id());
            }
        }

        #[cfg(feature = "have_fpu")]
        lazy_fpu_restore(get_currenct_thread());
        asm!(
                "mov     sp, {}                     \n",

                /* Restore thread's SPSR, LR, and SP */
                "ldp     x21, x22, [sp, #31 * 8] \n",
                "ldr     x23, [sp, #33 * 8]    \n",
                "msr     sp_el0, x21                \n",
        // #ifdef CONFIG_ARM_HYPERVISOR_SUPPORT
        //         "msr     elr_el2, x22               \n"
        //         "msr     spsr_el2, x23              \n"
        // #else
                "msr     elr_el1, x22               \n",
                "msr     spsr_el1, x23              \n",
        // #endif
                /* Restore remaining registers */
                "ldp     x0,  x1,  [sp, #16 * 0]    \n",
                "ldp     x2,  x3,  [sp, #16 * 1]    \n",
                "ldp     x4,  x5,  [sp, #16 * 2]    \n",
                "ldp     x6,  x7,  [sp, #16 * 3]    \n",
                "ldp     x8,  x9,  [sp, #16 * 4]    \n",
                "ldp     x10, x11, [sp, #16 * 5]    \n",
                "ldp     x12, x13, [sp, #16 * 6]    \n",
                "ldp     x14, x15, [sp, #16 * 7]    \n",
                "ldp     x16, x17, [sp, #16 * 8]    \n",
                "ldp     x18, x19, [sp, #16 * 9]    \n",
                "ldp     x20, x21, [sp, #16 * 10]   \n",
                "ldp     x22, x23, [sp, #16 * 11]   \n",
                "ldp     x24, x25, [sp, #16 * 12]   \n",
                "ldp     x26, x27, [sp, #16 * 13]   \n",
                "ldp     x28, x29, [sp, #16 * 14]   \n",
                "ldr     x30, [sp, #30 * 8]          \n",
                "eret",
                in(reg) get_currenct_thread().tcbArch.raw_ptr()
            );
    }
    panic!("unreachable")
}

#[inline]
#[no_mangle]
pub fn fastpath_restore(_badge: usize, _msgInfo: usize, cur_thread: *mut tcb_t) {
    unsafe {
        #[cfg(feature = "enable_smp")]
        {
            clh_lock_release(cpu_id());
        }

        (*cur_thread).tcbArch.load_thread_local();
        #[cfg(feature = "have_fpu")]
        {
            lazy_fpu_restore(get_currenct_thread());
        }
        asm!(
            "mov     x0, {0}                     \n",
            "mov     x1, {1}                     \n",
            "mov     sp, {2}                     \n",
            /* Restore thread's SPSR, LR, and SP */
            "ldp     x21, x22, [sp, #31 * 8]  \n",
            "ldr     x23, [sp, #33 * 8]     \n",
            "msr     sp_el0, x21                \n",
            // #ifdef CONFIG_ARM_HYPERVISOR_SUPPORT
            // 		"msr     elr_el2, x22               \n"
            // 		"msr     spsr_el2, x23              \n"
            // #else
            "msr     elr_el1, x22               \n",
            "msr     spsr_el1, x23              \n",
            // #endif

            /* Restore remaining registers */
            "ldp     x2,  x3,  [sp, #16 * 1]    \n",
            "ldp     x4,  x5,  [sp, #16 * 2]    \n",
            "ldp     x6,  x7,  [sp, #16 * 3]    \n",
            "ldp     x8,  x9,  [sp, #16 * 4]    \n",
            "ldp     x10, x11, [sp, #16 * 5]    \n",
            "ldp     x12, x13, [sp, #16 * 6]    \n",
            "ldp     x14, x15, [sp, #16 * 7]    \n",
            "ldp     x16, x17, [sp, #16 * 8]    \n",
            "ldp     x18, x19, [sp, #16 * 9]    \n",
            "ldp     x20, x21, [sp, #16 * 10]   \n",
            "ldp     x22, x23, [sp, #16 * 11]   \n",
            "ldp     x24, x25, [sp, #16 * 12]   \n",
            "ldp     x26, x27, [sp, #16 * 13]   \n",
            "ldp     x28, x29, [sp, #16 * 14]   \n",
            "ldr     x30, [sp, #30 * 8]           \n",
            "eret                                 ",
            in(reg) _badge,
            in(reg) _msgInfo,
            in(reg) (*cur_thread).tcbArch.raw_ptr()
        );
    }
    panic!("unreachable")
}

#[no_mangle]
pub fn c_handle_interrupt() {
    // log::debug!("c_handle_interrupt");
    // if hart_id() != 0 {
    //     debug!("c_handle_interrupt");
    // }
    entry_hook();

    #[cfg(feature = "enable_smp")]
    {
        use sel4_common::arch::config::IRQ_REMOTE_CALL_IPI;
        if get_active_irq() != IRQ_REMOTE_CALL_IPI {
            unsafe {
                clh_lock_acquire(cpu_id(), true);
            }
        }
    }
    // sel4_common::println!("c_handle_interrupt");
    handle_interrupt_entry();
    restore_user_context();
}

#[no_mangle]
pub fn c_handle_syscall(_cptr: usize, _msgInfo: usize, syscall: usize) {
    #[cfg(feature = "enable_smp")]
    unsafe {
        clh_lock_acquire(cpu_id(), false);
    }
    entry_hook();
    // if hart_id() == 0 {
    //     debug!("c_handle_syscall: syscall: {},", syscall as isize);
    // }
    // sel4_common::println!("c handle syscall");
    slow_path(syscall);
    // debug!("c_handle_syscall complete");
}

/// This function should be the first thing called from after entry.
/// This function Save TPIDR(TLS) in aarch64.
#[inline]
pub fn entry_hook() {
    get_currenct_thread().tcbArch.save_thread_local();
}

#[no_mangle]
#[cfg(feature = "build_binary")]
pub fn c_handle_fastpath_call(cptr: usize, msgInfo: usize) -> ! {
    // TODO: support NODE_LOCK_SYS for smp mode
    entry_hook();
    use crate::kernel::fastpath::fastpath_call;
    fastpath_call(cptr, msgInfo);
    unreachable!()
}

#[no_mangle]
#[cfg(feature = "build_binary")]
#[cfg(not(feature = "kernel_mcs"))]
pub fn c_handle_fastpath_reply_recv(cptr: usize, msgInfo: usize) -> ! {
    // TODO: support NODE_LOCK_SYS for smp mode
    entry_hook();
    crate::kernel::fastpath::fastpath_reply_recv(cptr, msgInfo);
    unreachable!()
}

#[no_mangle]
#[cfg(feature = "build_binary")]
#[cfg(feature = "kernel_mcs")]
pub fn c_handle_fastpath_reply_recv(cptr: usize, msgInfo: usize, reply: usize) -> ! {
    // TODO: support NODE_LOCK_SYS for smp mode
    entry_hook();
    crate::kernel::fastpath::fastpath_reply_recv(cptr, msgInfo, reply);
    unreachable!()
}

#[no_mangle]
#[cfg(feature = "build_binary")]
pub fn c_handle_undefined_instruction() -> ! {
    // TODO: support NODE_LOCK_SYS for smp mode
    entry_hook();

    // Only support aarch64
    // No hypervisor support

    let esr: usize;
    unsafe { asm!("mrs {}, ESR_EL1", out(reg) esr) }
    super::exception::handleUserLevelFault(esr, 0);
    restore_user_context();
    unreachable!()
}
#[cfg(feature = "have_fpu")]
#[no_mangle]
pub fn c_handle_enfp() -> ! {
    use super::fpu::handle_fpu_fault;
    entry_hook();
    unsafe { handle_fpu_fault() };
    restore_user_context();
    unreachable!()
}
