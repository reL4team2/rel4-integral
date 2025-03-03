#![allow(unused)]

use core::{arch::asm, intrinsics::{likely, unlikely}};

use sel4_common::arch::arch_tcb::FPUState;
use sel4_task::{get_currenct_thread, tcb_t};

use crate::config::CONFIG_FPU_MAX_RESTORES_SINCE_SWITCH;

#[no_mangle]
pub static mut ksActiveFPUStatePtr: Option<usize> = None;

#[no_mangle]
pub static mut ksFPURestoresSinceSwitch: usize = 0;

// TODO: support smp
static mut isFPUEnabledCached: bool = false;

#[inline]
unsafe fn save_fpu_state(dest: usize) {
    asm!(
        // SIMD and floating-point register file
        "stp     q0, q1, [{0}, #16 * 0]",
        "stp     q2, q3, [{0}, #16 * 2]",
        "stp     q4, q5, [{0}, #16 * 4]",
        "stp     q6, q7, [{0}, #16 * 6]",
        "stp     q8, q9, [{0}, #16 * 8]",
        "stp     q10, q11, [{0}, #16 * 10]",
        "stp     q12, q13, [{0}, #16 * 12]",
        "stp     q14, q15, [{0}, #16 * 14]",
        "stp     q16, q17, [{0}, #16 * 16]",
        "stp     q18, q19, [{0}, #16 * 18]",
        "stp     q20, q21, [{0}, #16 * 20]",
        "stp     q22, q23, [{0}, #16 * 22]",
        "stp     q24, q25, [{0}, #16 * 24]",
        "stp     q26, q27, [{0}, #16 * 26]",
        "stp     q28, q29, [{0}, #16 * 28]",
        "stp     q30, q31, [{0}, #16 * 30]",

        // FP control and status registers
        "mrs     x8, fpsr",
        "str     x8, [{0}, #16 * 32]",
        // "mrs     x8, fpcr",
        // "str     x8, [{0}, #516]",
        in(reg) (dest as usize),
        options(nostack, preserves_flags)
    );
}

#[inline]
unsafe fn load_fpu_state(dest: usize) {
    asm!(
        // SIMD and floating-point register file
        "ldp     q0, q1, [{0}, #16 * 0]",
        "ldp     q2, q3, [{0}, #16 * 2]",
        "ldp     q4, q5, [{0}, #16 * 4]",
        "ldp     q6, q7, [{0}, #16 * 6]",
        "ldp     q8, q9, [{0}, #16 * 8]",
        "ldp     q10, q11, [{0}, #16 * 10]",
        "ldp     q12, q13, [{0}, #16 * 12]",
        "ldp     q14, q15, [{0}, #16 * 14]",
        "ldp     q16, q17, [{0}, #16 * 16]",
        "ldp     q18, q19, [{0}, #16 * 18]",
        "ldp     q20, q21, [{0}, #16 * 20]",
        "ldp     q22, q23, [{0}, #16 * 22]",
        "ldp     q24, q25, [{0}, #16 * 24]",
        "ldp     q26, q27, [{0}, #16 * 26]",
        "ldp     q28, q29, [{0}, #16 * 28]",
        "ldp     q30, q31, [{0}, #16 * 30]",

        // FP control and status registers
        "ldr     x8, [{0}, #16 * 32]",
        "msr     fpsr, x8",
        // "ldr     x8, [{0}, #16 * 32 + 4]    \n",
        // "msr     fpcr, x8",
        in(reg) (dest as usize),
        options(nostack, preserves_flags)
    );
}

#[inline]
unsafe fn enableFpu() -> usize {
    // TODO: don't support EL2
    let mut cpacr: usize = 0;
    asm!(
        "mrs {0}, cpacr_el1",
        "orr {0}, {0}, #(0x3 << 20)",
        "msr cpacr_el1, {0}",
        "isb",
        inout(reg) cpacr,
    );

    isFPUEnabledCached = true;
    cpacr
}

#[inline]
unsafe fn disableFpu() {
    asm!(
        "mrs x8, cpacr_el1",
        "bic x8, x8, #(0x3 << 20)",
        "orr x8, x8, #(0x1 << 20)",
        "msr cpacr_el1, x8",
        "isb"
    );
    isFPUEnabledCached = false
}

#[inline]
#[allow(unused)]
pub(crate) unsafe fn isFpuEnable() -> bool {
    return isFPUEnabledCached;
}

#[inline]
unsafe fn switchLocalFpuOwner(new_owner: Option<*const FPUState>) {
    enableFpu();
    if let Some(ptr) = ksActiveFPUStatePtr {
        save_fpu_state(ptr);
    }
    
    if let Some(owner) = new_owner {
        ksFPURestoresSinceSwitch = 0;
        load_fpu_state(owner as *const FPUState as usize);
        ksActiveFPUStatePtr = Some(owner as usize);
    } else {
        ksActiveFPUStatePtr = None;
        disableFpu();
    }
}

#[inline]
#[allow(unused)]
pub(crate) unsafe fn handleFPUFault() {
    let new_owner = get_currenct_thread().tcbArch.fpu_state_ptr();
    switchLocalFpuOwner(Some(new_owner ));
}

#[inline(always)]
unsafe fn nativeThreadUsingFPU(thread: &mut tcb_t) -> bool {
    if let Some(ptr) = ksActiveFPUStatePtr {
        return thread.tcbArch.fpu_state_ptr() as usize == ptr;
    }
    
    false
}

#[inline(always)]
#[allow(unused)]
pub(crate) unsafe fn lazyFPURestore(thread: &mut tcb_t) {
    if let Some(_) = ksActiveFPUStatePtr {
        if unlikely( ksFPURestoresSinceSwitch > CONFIG_FPU_MAX_RESTORES_SINCE_SWITCH) {
            switchLocalFpuOwner(None);
            ksFPURestoresSinceSwitch = 0
        } else {
            if likely(nativeThreadUsingFPU(thread)) {
                enableFpu();
            } else {
                disableFpu();
            }
            ksFPURestoresSinceSwitch += 1;
        }
    }
}
