#![allow(unused)]

use core::{
    arch::asm,
    intrinsics::{likely, unlikely},
};

use sel4_common::arch::arch_tcb::FPUState;
use sel4_task::{get_currenct_thread, tcb_t};

use crate::config::CONFIG_FPU_MAX_RESTORES_SINCE_SWITCH;

#[no_mangle]
pub static mut ksActiveFPUState: usize = 0;

#[no_mangle]
pub static mut ksFPURestoresSinceSwitch: usize = 0;

// TODO: support smp
static mut isFPUEnabledCached: bool = false;

core::arch::global_asm!(include_str!("fpu.S"));
extern "C" {
    pub fn save_fpu_state(dest: usize, dest_fpsr: usize);
    pub fn load_fpu_state(src: usize, src_fpsr: usize);
}

#[inline]
pub(crate) unsafe fn enableFpu() -> usize {
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
pub(crate) unsafe fn disableFpu() {
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
pub unsafe fn isFpuEnable() -> bool {
    return isFPUEnabledCached;
}

#[inline]
unsafe fn switchLocalFpuOwner(new_owner: usize) {
    unsafe {
        enableFpu();
        if ksActiveFPUState != 0 {
            save_fpu_state(ksActiveFPUState, ksActiveFPUState + 16 * 32);
        }

        if new_owner != 0 {
            ksFPURestoresSinceSwitch = 0;
            load_fpu_state(
                new_owner as *const FPUState as usize,
                new_owner as *const FPUState as usize + 16 * 32,
            );
        } else {
            disableFpu();
        }
        ksActiveFPUState = new_owner;
    }
}

#[inline]
#[allow(unused)]
pub(crate) unsafe fn handleFPUFault() {
    let new_owner = get_currenct_thread().tcbArch.fpu_state_ptr();
    switchLocalFpuOwner(new_owner as usize);
}

#[inline(always)]
unsafe fn nativeThreadUsingFPU(thread: &mut tcb_t) -> bool {
    return thread.tcbArch.fpu_state_ptr() as usize == ksActiveFPUState;
}

#[inline(always)]
pub fn fpuThreadDelete(thread: &mut tcb_t) {
    unsafe {
        if nativeThreadUsingFPU(thread) {
            switchLocalFpuOwner(0);
        }
    }
}

#[inline(always)]
#[allow(unused)]
pub unsafe fn lazyFPURestore(thread: &mut tcb_t) {
    if ksActiveFPUState != 0 {
        if unlikely(ksFPURestoresSinceSwitch > CONFIG_FPU_MAX_RESTORES_SINCE_SWITCH) {
            switchLocalFpuOwner(0);
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
