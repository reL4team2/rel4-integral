#![allow(unused)]

use core::{
    arch::asm,
    intrinsics::{likely, unlikely},
};

use sel4_common::arch::arch_tcb::FPUState;
use sel4_task::{
    get_currenct_thread, get_current_active_fpu_state, get_current_fpu_restore_since_switch,
    set_current_active_fpu_state, set_current_fpu_restore_since_switch, tcb_t,
};

use sel4_common::sel4_config::CONFIG_FPU_MAX_RESTORES_SINCE_SWITCH;
use sel4_common::utils::cpu_id;

#[cfg(feature = "enable_smp")]
use crate::smp::ipi::remote_switch_fpu_owner;

// TODO: support smp
static mut is_fpu_enabled_cached: bool = false;

core::arch::global_asm!(include_str!("fpu.S"));
extern "C" {
    pub fn save_fpu_state(dest: usize, dest_fpsr: usize);
    pub fn load_fpu_state(src: usize, src_fpsr: usize);
}

#[inline]
pub(crate) unsafe fn enable_fpu() -> usize {
    // TODO: don't support EL2
    let mut cpacr: usize = 0;
    asm!(
        "mrs {0}, cpacr_el1",
        "orr {0}, {0}, #(0x3 << 20)",
        "msr cpacr_el1, {0}",
        "isb",
        inout(reg) cpacr,
    );

    is_fpu_enabled_cached = true;
    cpacr
}

#[inline]
pub(crate) unsafe fn disable_fpu() {
    asm!(
        "mrs x8, cpacr_el1",
        "bic x8, x8, #(0x3 << 20)",
        "orr x8, x8, #(0x1 << 20)",
        "msr cpacr_el1, x8",
        "isb"
    );
    is_fpu_enabled_cached = false
}

#[inline]
#[allow(unused)]
pub unsafe fn is_fpu_enable() -> bool {
    return is_fpu_enabled_cached;
}

#[inline]
#[no_mangle]
pub unsafe fn switch_local_fpu_owner(new_owner: usize) {
    unsafe {
        enable_fpu();
        let ksActiveFPUState = get_current_active_fpu_state();
        if ksActiveFPUState != 0 {
            save_fpu_state(ksActiveFPUState, ksActiveFPUState + 16 * 32);
        }

        if new_owner != 0 {
            set_current_fpu_restore_since_switch(0);
            load_fpu_state(
                new_owner as *const FPUState as usize,
                new_owner as *const FPUState as usize + 16 * 32,
            );
        } else {
            disable_fpu();
        }
        set_current_active_fpu_state(new_owner);
    }
}

#[cfg(feature = "enable_smp")]
pub fn switch_fpu_owner(new_owner: usize, cpu: usize) {
    if cpu != cpu_id() {
        remote_switch_fpu_owner(new_owner, cpu);
    } else {
        unsafe { switch_local_fpu_owner(new_owner) };
    }
}

#[inline]
#[allow(unused)]
pub(crate) unsafe fn handle_fpu_fault() {
    let new_owner = get_currenct_thread().tcbArch.fpu_state_ptr();
    switch_local_fpu_owner(new_owner as usize);
}

#[inline(always)]
unsafe fn native_thread_using_fpu(thread: &mut tcb_t) -> bool {
    return thread.tcbArch.fpu_state_ptr() as usize == get_current_active_fpu_state();
}

#[cfg(feature = "enable_smp")]
#[inline(always)]
pub fn fpu_thread_delete(thread: &mut tcb_t) {
    unsafe {
        if native_thread_using_fpu(thread) {
            switch_fpu_owner(0, thread.tcbAffinity);
        }
    }
}

#[cfg(not(feature = "enable_smp"))]
#[inline(always)]
pub fn fpu_thread_delete(thread: &mut tcb_t) {
    unsafe {
        if native_thread_using_fpu(thread) {
            switch_local_fpu_owner(0);
        }
    }
}

#[inline(always)]
#[allow(unused)]
pub unsafe fn lazy_fpu_restore(thread: &mut tcb_t) {
    if get_current_active_fpu_state() != 0 {
        let current_fpu_restore = get_current_fpu_restore_since_switch();
        if unlikely(current_fpu_restore > CONFIG_FPU_MAX_RESTORES_SINCE_SWITCH) {
            switch_local_fpu_owner(0);
            set_current_fpu_restore_since_switch(0);
        } else {
            if likely(native_thread_using_fpu(thread)) {
                enable_fpu();
            } else {
                disable_fpu();
            }
            set_current_fpu_restore_since_switch(current_fpu_restore + 1);
        }
    }
}
