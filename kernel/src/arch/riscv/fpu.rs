#![allow(unused)]

use core::{
    arch::asm,
    intrinsics::{likely, unlikely},
};

use sel4_common::arch::arch_tcb::FPUState;
use sel4_task::{
    get_currenct_thread, tcb_t, get_current_active_fpu_state, set_current_active_fpu_state,
    get_current_fpu_restore_since_switch, set_current_fpu_restore_since_switch
};
use sel4_common::arch::ArchReg;
use sel4_common::sel4_config::CONFIG_FPU_MAX_RESTORES_SINCE_SWITCH;
use sel4_common::utils::cpu_id;

#[cfg(feature = "enable_smp")]
use crate::smp::ipi::remote_switch_fpu_owner;

const SSTATUS_FS: usize = 0x00006000;
const SSTATUS_FS_CLEAN: u32 = 0x00004000;
const SSTATUS_FS_INITIAL: u32 = 0x00002000;
const SSTATUS_FS_DIRTY: u32 = 0x00006000;

// extern "C" {
//     pub fn saveFpuState(dest: usize);
//     pub fn loadFpuState(src: usize);
// }
// TODO: support smp
static mut IS_FPU_ENABLE_CACHED: bool = false;

#[cfg(feature = "riscv_ext_d")]
#[inline]
pub fn save_fpu_state(dest: usize) {
    unsafe {
        set_fs_clean();
        asm!(
            "fsd f0, 0*8({0})",
            "fsd f1, 1*8({0})",
            "fsd f2, 2*8({0})",
            "fsd f3, 3*8({0})",
            "fsd f4, 4*8({0})",
            "fsd f5, 5*8({0})",
            "fsd f6, 6*8({0})",
            "fsd f7, 7*8({0})",
            "fsd f8, 8*8({0})",
            "fsd f9, 9*8({0})",
            "fsd f10, 10*8({0})",
            "fsd f11, 11*8({0})",
            "fsd f12, 12*8({0})",
            "fsd f13, 13*8({0})",
            "fsd f14, 14*8({0})",
            "fsd f15, 15*8({0})",
            "fsd f16, 16*8({0})",
            "fsd f17, 17*8({0})",
            "fsd f18, 18*8({0})",
            "fsd f19, 19*8({0})",
            "fsd f20, 20*8({0})",
            "fsd f21, 21*8({0})",
            "fsd f22, 22*8({0})",
            "fsd f23, 23*8({0})",
            "fsd f24, 24*8({0})",
            "fsd f25, 25*8({0})",
            "fsd f26, 26*8({0})",
            "fsd f27, 27*8({0})",
            "fsd f28, 28*8({0})",
            "fsd f29, 29*8({0})",
            "fsd f30, 30*8({0})",
            "fsd f31, 31*8({0})",
            in(reg) dest
        );
        (*(dest as *mut FPUState)).fcsr = read_fcsr();
    }
}
#[cfg(feature = "riscv_ext_f")]
#[inline]
pub fn save_fpu_state(dest: usize) {
    unsafe {
        set_fs_clean();
        asm!(
            "fsw f0, 0*4({0})",
            "fsw f1, 1*4({0})",
            "fsw f2, 2*4({0})",
            "fsw f3, 3*4({0})",
            "fsw f4, 4*4({0})",
            "fsw f5, 5*4({0})",
            "fsw f6, 6*4({0})",
            "fsw f7, 7*4({0})",
            "fsw f8, 8*4({0})",
            "fsw f9, 9*4({0})",
            "fsw f10, 10*4({0})",
            "fsw f11, 11*4({0})",
            "fsw f12, 12*4({0})",
            "fsw f13, 13*4({0})",
            "fsw f14, 14*4({0})",
            "fsw f15, 15*4({0})",
            "fsw f16, 16*4({0})",
            "fsw f17, 17*4({0})",
            "fsw f18, 18*4({0})",
            "fsw f19, 19*4({0})",
            "fsw f20, 20*4({0})",
            "fsw f21, 21*4({0})",
            "fsw f22, 22*4({0})",
            "fsw f23, 23*4({0})",
            "fsw f24, 24*4({0})",
            "fsw f25, 25*4({0})",
            "fsw f26, 26*4({0})",
            "fsw f27, 27*4({0})",
            "fsw f28, 28*4({0})",
            "fsw f29, 29*4({0})",
            "fsw f30, 30*4({0})",
            "fsw f31, 31*4({0})",
            in(reg) dest
        );
        (*(dest as *mut FPUState)).fcsr = read_fcsr();
    }
}
#[cfg(feature = "riscv_ext_d")]
#[inline]
pub fn load_fpu_state(src: usize) {
    unsafe {
        set_fs_clean();
        asm!(
            "fld f0, 0*8({0})",
            "fld f1, 1*8({0})",
            "fld f2, 2*8({0})",
            "fld f3, 3*8({0})",
            "fld f4, 4*8({0})",
            "fld f5, 5*8({0})",
            "fld f6, 6*8({0})",
            "fld f7, 7*8({0})",
            "fld f8, 8*8({0})",
            "fld f9, 9*8({0})",
            "fld f10, 10*8({0})",
            "fld f11, 11*8({0})",
            "fld f12, 12*8({0})",
            "fld f13, 13*8({0})",
            "fld f14, 14*8({0})",
            "fld f15, 15*8({0})",
            "fld f16, 16*8({0})",
            "fld f17, 17*8({0})",
            "fld f18, 18*8({0})",
            "fld f19, 19*8({0})",
            "fld f20, 20*8({0})",
            "fld f21, 21*8({0})",
            "fld f22, 22*8({0})",
            "fld f23, 23*8({0})",
            "fld f24, 24*8({0})",
            "fld f25, 25*8({0})",
            "fld f26, 26*8({0})",
            "fld f27, 27*8({0})",
            "fld f28, 28*8({0})",
            "fld f29, 29*8({0})",
            "fld f30, 30*8({0})",
            "fld f31, 31*8({0})",
            in(reg) src
        );
        write_fcsr((*(src as *mut FPUState)).fcsr);
    }
}
#[cfg(feature = "riscv_ext_f")]
#[inline]
pub fn load_fpu_state(src: usize) {
    unsafe {
        set_fs_clean();
        asm!(
            "fld f0, 0*4({0})",
            "fld f1, 1*4({0})",
            "fld f2, 2*4({0})",
            "fld f3, 3*4({0})",
            "fld f4, 4*4({0})",
            "fld f5, 5*4({0})",
            "fld f6, 6*4({0})",
            "fld f7, 7*4({0})",
            "fld f8, 8*4({0})",
            "fld f9, 9*4({0})",
            "fld f10, 10*4({0})",
            "fld f11, 11*4({0})",
            "fld f12, 12*4({0})",
            "fld f13, 13*4({0})",
            "fld f14, 14*4({0})",
            "fld f15, 15*4({0})",
            "fld f16, 16*4({0})",
            "fld f17, 17*4({0})",
            "fld f18, 18*4({0})",
            "fld f19, 19*4({0})",
            "fld f20, 20*4({0})",
            "fld f21, 21*4({0})",
            "fld f22, 22*4({0})",
            "fld f23, 23*4({0})",
            "fld f24, 24*4({0})",
            "fld f25, 25*4({0})",
            "fld f26, 26*4({0})",
            "fld f27, 27*4({0})",
            "fld f28, 28*4({0})",
            "fld f29, 29*4({0})",
            "fld f30, 30*4({0})",
            "fld f31, 31*4({0})",
            in(reg) src
        );
        write_fcsr((*(src as *mut FPUState)).fcsr);
    }
}

#[inline]
pub(crate) fn read_fcsr() -> u32 {
    let fcsr: u32;
    unsafe {
        asm!("csrr {0}, fcsr", out(reg) fcsr);
    }
    fcsr
}
#[inline]
pub(crate) fn write_fcsr(value: u32) {
    unsafe {
        asm!("csrw fcsr, {0}", in(reg) value);
    }
}
#[inline]
pub unsafe fn set_fs_clean() {
    asm!("csrs sstatus, {0}", in(reg) SSTATUS_FS_CLEAN);
}
#[inline]
pub unsafe fn set_fs_initial() {
    asm!("csrs sstatus, {0}", in(reg) SSTATUS_FS_INITIAL);
}
#[inline]
pub unsafe fn set_fs_dirty() {
    asm!("csrs sstatus, {0}", in(reg) SSTATUS_FS_DIRTY);
}
#[inline]
pub unsafe fn set_fs_off() {
    asm!("csrc sstatus, {0}", in(reg) SSTATUS_FS);
}

#[inline]
pub(crate) unsafe fn enable_fpu() {
    IS_FPU_ENABLE_CACHED = true;
}

#[inline]
pub(crate) unsafe fn disable_fpu() {
    IS_FPU_ENABLE_CACHED = false
}

#[inline]
#[allow(unused)]
pub unsafe fn is_fpu_enable() -> bool {
    return IS_FPU_ENABLE_CACHED;
}
#[inline]
#[allow(unused)]
pub unsafe fn set_tcb_fs_state(tcb: &mut tcb_t, enabled: bool) {
    let mut sstatus: usize = tcb.tcbArch.get_register(ArchReg::SSTATUS);
    sstatus &= !SSTATUS_FS;
    if enabled {
        sstatus |= SSTATUS_FS_CLEAN as usize;
    }
    tcb.tcbArch.set_register(ArchReg::SSTATUS, sstatus);
}

#[inline]
#[no_mangle]
pub unsafe fn switch_local_fpu_owner(new_owner: usize) {
    unsafe {
        enable_fpu();
        let ksActiveFPUState = get_current_active_fpu_state(); 
        if ksActiveFPUState != 0 {
            save_fpu_state(ksActiveFPUState);
        }

        if new_owner != 0 {
            set_current_fpu_restore_since_switch(0);
            load_fpu_state(new_owner as *const FPUState as usize);
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
#[inline]
pub fn init_fpu() {
    unsafe {
        set_fs_clean();
        write_fcsr(0);
        disable_fpu();
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
