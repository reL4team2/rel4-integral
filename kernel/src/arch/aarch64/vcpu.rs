use core::intrinsics::likely;

use aarch64_cpu::{
    asm::barrier,
    registers::{Readable, Writeable, HCR_EL2, ID_AA64MMFR0_EL1, SCTLR_EL1, VTCR_EL2},
};
use sel4_task::tcb_t;
use sel4_task::{get_currenct_thread, set_thread_state, ThreadState};

use sel4_common::arch::MessageLabel;
use sel4_common::structures::{exception_t, seL4_IPCBuffer};
use sel4_cspace::interface::cte_t;

use super::arm_gic::gic_v2::{
    gic_vcpu_num_list_regs, get_gic_vcpu_ctrl_apr, get_gic_vcpu_ctrl_eisr0,
    get_gic_vcpu_ctrl_eisr1, get_gic_vcpu_ctrl_hcr, get_gic_vcpu_ctrl_lr, get_gic_vcpu_ctrl_misr,
    get_gic_vcpu_ctrl_vmcr, set_gic_vcpu_ctrl_apr, set_gic_vcpu_ctrl_hcr,
    set_gic_vcpu_ctrl_lr, set_gic_vcpu_ctrl_vmcr,
};
use crate::interrupt::mask_interrupt;

const VMCS_SIZE: usize = 4096;
const IOBITMAP_SIZE: usize = 8192;

/// TODO: GIC_V2 is 16, and GIC_V3 is 64
const GIC_VCPU_MAX_NUM_LR: usize = 16;

const SCTLR_DEFAULT: u64 = 0xc5187c;
const ACTLR_DEFAULT: u64 = 0x40;

/// TODO: read this irq number dynamically.
const INTERRUPT_VTIMER_EVENT: usize = 2;

/// HCR_EL2 <https://developer.arm.com/documentation/ddi0601/2025-06/AArch64-Registers/HCR-EL2--Hypervisor-Configuration-Register>
const HCR_COMMON: u64 = HCR_EL2::VM::SET.value
    | HCR_EL2::RW::EL1IsAarch64.value
    | HCR_EL2::AMO::SET.value
    | HCR_EL2::IMO::EnableVirtualIRQ.value
    | HCR_EL2::FMO::EnableVirtualFIQ.value
    | HCR_EL2::TSC::EnableTrapEl1SmcToEl2.value;

const HCR_NATIVE: u64 = HCR_COMMON
    | HCR_EL2::TGE::EnableTrapGeneralExceptionsToEl2.value
    | HCR_EL2::SWIO::SET.value
    | bit!(12)  // DC: Default Cacheability – disables EL0 Stage‑1, forces VA→IPA passthrough
    // TVM(26) | TTLB(25) | TAC(21)
    | bits!(26, 25, 21);

// TWE(14) | TWI(13)
const HCR_VCPU: u64 = HCR_COMMON | bits!(14, 13);

/// Global state: currently active VCPU (per-core).
static mut ARM_HS_CUR_VCPU: usize = 0;

/// Global state: whether the currently loaded VCPU is active in hardware.
static mut ARM_HS_VCPU_ACTIVE: bool = false;

/// Number of list registers from VGIC VTR (cached at boot).
#[cfg(feature = "hypervisor")]
static mut GIC_NUM_LIST_REGS: usize = 0;

/// Check if a VCPU is currently active (for IPA→PA translation in VMFault).
#[cfg(feature = "hypervisor")]
#[inline]
pub fn is_vcpu_active() -> bool {
    unsafe { ARM_HS_VCPU_ACTIVE }
}

// ---------------------------------------------------------------------------
// Boot-time initialization
// ---------------------------------------------------------------------------

/// Armv8 init vcpu in boot stage
///
/// ID_AA64MMFR0_EL1: <https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64MMFR0-EL1--AArch64-Memory-Model-Feature-Register-0>
/// VTCR_EL2: <https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/VTCR-EL2--Virtualization-Translation-Control-Register>
pub fn armv_vcpu_boot_init() {
    if ID_AA64MMFR0_EL1.matches_all(ID_AA64MMFR0_EL1::TGran4::NotSupported) {
        panic!("Processor doesn't support 4KB");
    }
    // Select VTCR_EL2 configuration based on stage-2 starting level:
    // - 40-bit (feature "pa_40bit"): T0SZ=24, PS=1TB,  SL0=1 (3-level s2)
    // - 44-bit (default):        T0SZ=20, PS=16TB, SL0=2 (4-level s2)
    #[cfg(all(feature = "pa_40bit", feature = "hypervisor"))]
    let vtcr_val = {
        VTCR_EL2::T0SZ.val(24)
            + VTCR_EL2::PS::PA_40B_1TB
            + VTCR_EL2::SL0.val(1)
    };
    #[cfg(not(all(feature = "pa_40bit", feature = "hypervisor")))]
    let vtcr_val = {
        VTCR_EL2::T0SZ.val(20)
            + VTCR_EL2::PS::PA_44B_16TB
            + VTCR_EL2::SL0.val(2)
    };
    let vtcr_val = (vtcr_val
        + VTCR_EL2::IRGN0::NormalWBRAWA
        + VTCR_EL2::ORGN0::NormalWBRAWA
        + VTCR_EL2::SH0::Inner
        + VTCR_EL2::TG0::Granule4KB)
        .value
        | (1u64 << 31); // RES1: bit 31 must be 1 (ARM ARM)
    unsafe { core::arch::asm!("msr vtcr_el2, {}", in(reg) vtcr_val) };
    barrier::dsb(barrier::SY);
}

pub fn vcpu_boot_init() {
    armv_vcpu_boot_init();
    // Set SCTLR_EL1 to a known default. Mirrors C kernel's setSCTLR(SCTLR_DEFAULT).
    SCTLR_EL1.set(SCTLR_DEFAULT);
    barrier::isb(barrier::SY);
    // Set HCR_EL2 for native (non-VCPU) execution.
    // TGE=1 traps EL0 execution to EL2, VM=1 enables stage-2 translation.
    // Without HCR_EL2 properly configured, ERET to EL0 faults on real hardware.
    HCR_EL2.set(HCR_NATIVE);
    barrier::isb(barrier::SY);
    // Initialize VGIC: cache list register count from VTR
    unsafe {
        GIC_NUM_LIST_REGS = gic_vcpu_num_list_regs();
    }
    if unsafe { GIC_NUM_LIST_REGS } > GIC_VCPU_MAX_NUM_LR {
        log::warn!("VGIC reports more list registers than we support. Truncating.");
        unsafe { GIC_NUM_LIST_REGS = GIC_VCPU_MAX_NUM_LR; }
    }
    vcpu_disable(0 as _);
    unsafe {
        ARM_HS_CUR_VCPU = 0;
        ARM_HS_VCPU_ACTIVE = false;
    }
}

// ---------------------------------------------------------------------------
// VCPU enable / disable
// ---------------------------------------------------------------------------

pub fn vcpu_disable(vcpu: usize) {
    barrier::dsb(barrier::SY);

    if likely(vcpu != 0) {
        use aarch64_cpu::registers::{CPACR_EL1, Readable};
        let vcpu = unsafe { &mut *(vcpu as *mut VCpu) };
        // Save VGIC HCR (soon to be off)
        vcpu.vgic.hcr = get_gic_vcpu_ctrl_hcr();
        vcpu.regs.sctlr = SCTLR_EL1.get();
        vcpu.regs.cpacr = CPACR_EL1.get();
    }

    // Turn off the VGIC
    set_gic_vcpu_ctrl_hcr(0);
    barrier::isb(barrier::SY);

    SCTLR_EL1.set(SCTLR_DEFAULT);
    barrier::isb(barrier::SY);
    // In native mode (no VCPU), enable FP/SIMD BEFORE TGE=1,
    // because afterwards CPACR_EL1 access redirects to CPTR_EL2.
    unsafe { crate::arch::aarch64::fpu::native_enable_fpu(); }
    HCR_EL2.set(HCR_NATIVE);
    if likely(vcpu != 0) {
        #[cfg(feature = "enable_smp")]
        mask_interrupt(true, INTERRUPT_VTIMER_EVENT);
    }
}

pub fn vcpu_enable(vcpu: &VCpu) {
    SCTLR_EL1.set(vcpu.regs.sctlr);
    HCR_EL2.set(HCR_VCPU);
    barrier::isb(barrier::SY);
    // Restore VGIC HCR
    set_gic_vcpu_ctrl_hcr(vcpu.vgic.hcr);
    // Restore CPACR_EL1 with FPEN=0b11 and update FPU state cache.
    unsafe { crate::arch::aarch64::fpu::vcpu_restore_fpu(vcpu.regs.cpacr); }
}

// ---------------------------------------------------------------------------
// VCPU save / restore
// ---------------------------------------------------------------------------

/// Save the currently active VCPU's state.
fn vcpu_save(vcpu: &mut VCpu, active: bool) {
    barrier::dsb(barrier::SY);
    if active {
        vcpu.regs.sctlr = SCTLR_EL1.get();
        vcpu.vgic.hcr = get_gic_vcpu_ctrl_hcr();
    }
    // Always save GIC control state regardless of active
    vcpu.vgic.vmcr = get_gic_vcpu_ctrl_vmcr();
    vcpu.vgic.apr = get_gic_vcpu_ctrl_apr();
    let lr_num = unsafe { GIC_NUM_LIST_REGS };
    for i in 0..lr_num {
        vcpu.vgic.lr[i] = get_gic_vcpu_ctrl_lr(i) as usize;
    }
}

/// Restore a VCPU's state making it ready to run.
fn vcpu_restore(vcpu: &VCpu) {
    // Turn off VGIC first
    set_gic_vcpu_ctrl_hcr(0);
    barrier::isb(barrier::SY);

    // Restore GIC control state
    set_gic_vcpu_ctrl_vmcr(vcpu.vgic.vmcr);
    set_gic_vcpu_ctrl_apr(vcpu.vgic.apr);
    let lr_num = unsafe { GIC_NUM_LIST_REGS };
    for i in 0..lr_num {
        set_gic_vcpu_ctrl_lr(i, vcpu.vgic.lr[i] as u32);
    }

    // Enable VCPU (set HCR_EL2.VM=1, HCR_VCPU, restore VGIC HCR)
    vcpu_enable(vcpu);
}

// ---------------------------------------------------------------------------
// VCPU switch (4-branch logic matching C kernel vcpu_switch)
// ---------------------------------------------------------------------------

/// Switch the active VCPU.
///
/// - If `new` != current: save old → restore new
/// - If `new` == NULL:   disable VCPU
/// - If `new` == current but inactive: re-enable
pub fn vcpu_switch(new: usize) {
    let cur = unsafe { ARM_HS_CUR_VCPU };
    let active = unsafe { ARM_HS_VCPU_ACTIVE };

    if likely(cur != new) {
        if new != 0 {
            // Switching to a new VCPU
            #[allow(unused_variables)]
            if cur != 0 {
                let cur_vcpu = unsafe { &mut *(cur as *mut VCpu) };
                vcpu_save(cur_vcpu, active);
            }
            let new_vcpu = unsafe { &*(new as *const VCpu) };
            vcpu_restore(new_vcpu);
            unsafe {
                ARM_HS_CUR_VCPU = new;
                ARM_HS_VCPU_ACTIVE = true;
            }
        } else if active {
            // new == NULL, disable VCPU
            let cur_vcpu = unsafe { &mut *(cur as *mut VCpu) };
            vcpu_disable(cur as usize);
            unsafe {
                ARM_HS_VCPU_ACTIVE = false;
            }
        }
    } else if !active && new != 0 {
        // Same VCPU, but VCPU is currently disabled → re-enable
        barrier::isb(barrier::SY);
        let vcpu = unsafe { &*(new as *const VCpu) };
        vcpu_enable(vcpu);
        unsafe {
            ARM_HS_VCPU_ACTIVE = true;
        }
    }
}

/// Invalidate active VCPU state without saving.
fn vcpu_invalidate_active() {
    if unsafe { ARM_HS_VCPU_ACTIVE } {
        vcpu_disable(0);
        unsafe {
            ARM_HS_VCPU_ACTIVE = false;
        }
    }
    unsafe {
        ARM_HS_CUR_VCPU = 0;
    }
}

// ---------------------------------------------------------------------------
// VCPU ↔ TCB association
// ---------------------------------------------------------------------------

/// Associate a VCPU with a TCB.
pub fn associate_vcpu_tcb(vcpu: &mut VCpu, tcb: &mut tcb_t) {
    if tcb.tcbArch.tcbVCPU != 0 {
        dissociate_vcpu_tcb(
            unsafe { &mut *(tcb.tcbArch.tcbVCPU as *mut VCpu) },
            tcb,
        );
    }
    if vcpu.tcb != 0 {
        let old_tcb = unsafe { &mut *(vcpu.tcb as *mut tcb_t) };
        old_tcb.tcbArch.tcbVCPU = 0;
    }
    tcb.tcbArch.tcbVCPU = vcpu as *mut VCpu as usize;
    vcpu.tcb = tcb as *mut tcb_t as usize;
}

/// Handle a VCPU fault (HSR from Stage-2 translation fault or VCPU exit).
/// Attempts armv-level handling (FPU traps, HVC), then sends VCPUFault to VMM.
/// Returns true if the fault was fully handled (FPU, HVC) and the caller
/// should NOT schedule+activate — just restore_user_context.
/// Returns false if the caller should proceed with schedule+activate.
pub unsafe fn handle_vcpu_fault(hsr: usize) -> bool {
    use crate::syscall::handle_fault;
    use sel4_common::ffi::current_fault;
    use sel4_task::{activateThread, get_currenct_thread, schedule};

    // Handle FPU/SIMD traps from lower EL (matches C kernel armv_handleVCPUFault)
    // ESR_EC = 0x07: trap FP/SIMD register access (CPTR_EL2.TFP=1)
    // ESR_EC = 0x18: trap access to CPACR_EL1 from EL1
    const ESR_EC_TFP: usize = 0x07;
    const ESR_EC_CPACR: usize = 0x18;
    let esr_ec = (hsr >> 26) & 0x3F;
    #[cfg(feature = "have_fpu")]
    {   
        let fpu_enabled = crate::arch::aarch64::fpu::is_fpu_enable();
        if (esr_ec == ESR_EC_TFP || esr_ec == ESR_EC_CPACR) && !fpu_enabled {
            unsafe { crate::arch::aarch64::fpu::handle_fpu_fault(); }
            return true;  // handled, caller should NOT schedule
        }
        // Log when FPU trap is ignored (fpu_enabled=true or unexpected EC)
        static mut FPU_IGNORE_COUNT: usize = 0;
        unsafe { FPU_IGNORE_COUNT += 1; }
        if unsafe { FPU_IGNORE_COUNT } & 0xFF == 0 {
            log::debug!("[FPU_IGNORE] cnt={} esr_ec={:#x} fpu_enabled={}", unsafe { FPU_IGNORE_COUNT }, esr_ec, fpu_enabled);
        }
    }

    // Try armv-level handling first
    if hsr == 0x2000000 {
        // UNKNOWN_FAULT: likely HVC from guest
        crate::arch::aarch64::exception::handleUserLevelFault(
            crate::arch::aarch64::instruction::get_esr(),
            0,
        );
        return true;  // handled, caller should NOT schedule
    }

    // Otherwise send VCPUFault to user-level VMM
    unsafe {
        current_fault = sel4_common::structures_gen::seL4_Fault_VCPUFault::new(hsr as u64).unsplay();
    }
    handle_fault(get_currenct_thread());
    schedule();
    activateThread();
    false
}

// ---------------------------------------------------------------------------
// VGIC maintenance interrupt handler
// ---------------------------------------------------------------------------

/// Handle VGIC maintenance interrupt (IRQ 25).
/// Reads MISR/EISR to determine which LRs need maintenance,
/// then delivers a VGICMaintenance fault to the user-level VMM.
#[cfg(feature = "hypervisor")]
pub fn vgic_maintenance() {
    use crate::syscall::handle_fault;
    use sel4_common::ffi::current_fault;
    use sel4_task::{activateThread, get_currenct_thread, schedule};

    if !unsafe { ARM_HS_VCPU_ACTIVE } {
        log::warn!("Received VGIC maintenance without active VCPU!");
        return;
    }

    let eisr0 = get_gic_vcpu_ctrl_eisr0();
    let eisr1 = get_gic_vcpu_ctrl_eisr1();
    let flags = get_gic_vcpu_ctrl_misr();

    // VGIC_MISR_EOI bit is 0
    if flags & 1 != 0 {
        let irq_idx: isize = if eisr0 != 0 {
            eisr0.trailing_zeros() as isize
        } else if eisr1 != 0 {
            eisr1.trailing_zeros() as isize + 32
        } else {
            -1
        };

        let lr_num = unsafe { GIC_NUM_LIST_REGS } as isize;
        if irq_idx == -1 || irq_idx >= lr_num {
            unsafe {
                current_fault =
                    sel4_common::structures_gen::seL4_Fault_VGICMaintenance::new(0, 0).unsplay();
            }
        } else {
            let virq = get_gic_vcpu_ctrl_lr(irq_idx as usize);
            // Clear the EIOIRQEN bit
            let updated = virq;
            set_gic_vcpu_ctrl_lr(irq_idx as usize, updated);
            // Sync shadow
            if unsafe { ARM_HS_CUR_VCPU } != 0 {
                let vcpu = unsafe { &mut *(ARM_HS_CUR_VCPU as *mut VCpu) };
                vcpu.vgic.lr[irq_idx as usize] = updated as usize;
            }
            unsafe {
                current_fault = sel4_common::structures_gen::seL4_Fault_VGICMaintenance::new(
                    irq_idx as u64, 1,
                )
                .unsplay();
            }
        }
    } else {
        unsafe {
            current_fault =
                sel4_common::structures_gen::seL4_Fault_VGICMaintenance::new(0, 0).unsplay();
        }
    }

    handle_fault(get_currenct_thread());
    schedule();
    activateThread();
}

// ---------------------------------------------------------------------------
// VPPI event handler
// ---------------------------------------------------------------------------

/// Handle a VPPI event interrupt.
#[cfg(feature = "hypervisor")]
#[allow(unused_imports)]
pub fn vppi_event(irq: u32) {
    use sel4_common::ffi::current_fault;
    use crate::syscall::handle_fault;
    use sel4_task::get_currenct_thread;

    if unsafe { !ARM_HS_VCPU_ACTIVE } {
        return;
    }

    mask_interrupt(true, irq as usize);
    // Mark VPPI as masked
    {
        let vcpu = unsafe { &mut *(ARM_HS_CUR_VCPU as *mut VCpu) };
        let idx = 0; // Simple: single VPPI event
        vcpu.vppi_masked[idx] = true;
    }

    unsafe {
        current_fault = sel4_common::structures_gen::seL4_Fault_VPPIEvent::new(irq as u64).unsplay();
    }

    let thread = get_currenct_thread();
    if thread.is_runnable() {
        handle_fault(thread);
    }
}

/// Dissociate a VCPU from a TCB.
pub fn dissociate_vcpu_tcb(vcpu: &mut VCpu, tcb: &mut tcb_t) {
    assert!(tcb.tcbArch.tcbVCPU == vcpu as *mut VCpu as usize);
    assert!(vcpu.tcb == tcb as *mut tcb_t as usize);

    if unsafe { ARM_HS_CUR_VCPU } == vcpu as *mut VCpu as usize {
        vcpu_invalidate_active();
    }
    tcb.tcbArch.tcbVCPU = 0;
    vcpu.tcb = 0;
}

// ---------------------------------------------------------------------------
// VCPU data structures
// ---------------------------------------------------------------------------

pub struct VTimer {
    last_pcount: u64,
}

struct VCpuRegSet {
    // System control registers EL1
    pub sctlr: u64,
    pub cpacr: u64,
    pub ttbr0: usize,
    pub ttbr1: usize,
    pub tcr: usize,
    pub mair: usize,
    pub amair: usize,
    pub cidr: usize,
    pub actlr: usize,

    // exception handling registers EL1
    pub afsr0: usize,
    pub afsr1: usize,
    pub esr: usize,
    pub far: usize,
    pub isr: usize,
    pub vbar: usize,

    // thread pointer/ID registers EL0/EL1
    pub tpidr_el1: usize,

    // Virtualisation Multiprocessor ID Register
    pub vmpidr_el2: usize,

    // general registers x0 to x30 have been saved by traps.S
    pub sp_el1: usize,
    pub elr_el1: usize,
    pub spsr_el1: usize,

    // generic timer registers, to be completed
    pub cntv_ctl: usize,
    pub cntv_cval: usize,
    pub cntv_off: usize,
    pub cntk_ctl_el1: usize,
}

struct GICVCpuIface {
    hcr: u32,
    vmcr: u32,
    apr: u32,
    _padding: u32,
    lr: [usize; GIC_VCPU_MAX_NUM_LR],
}

pub struct VCpu {
    /* TCB associated with this VCPU. */
    pub tcb: usize,
    pub vgic: GICVCpuIface,
    pub regs: VCpuRegSet,
    pub vppi_masked: [bool; 1],
    pub vtimer: VTimer,
}

// ---------------------------------------------------------------------------
// VCPU invocation decode
// ---------------------------------------------------------------------------

#[cfg(feature = "hypervisor")]
pub fn decode_vcpu_invocation(
    label: MessageLabel,
    length: usize,
    slot: &mut cte_t,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    use crate::kernel::boot::current_syscall_error;
    use crate::syscall::get_syscall_arg;
    use sel4_common::sel4_config::*;
    use sel4_common::structures_gen::cap;

    match label as usize {
        47 => { // ARMVCPUInjectIRQ
            // seL4_ARM_VCPU_InjectIRQ: MR0=virq, MR1=index
            let virq = get_syscall_arg(0, buffer);
            let index = get_syscall_arg(1, buffer);
            let vcpu_ptr = cap::cap_vcpu_cap(&slot.capability).get_capVCPUPtr() as usize;
            let vcpu = unsafe { &mut *(vcpu_ptr as *mut VCpu) };
            if index >= GIC_VCPU_MAX_NUM_LR {
                unsafe { current_syscall_error._type = SEL4_RANGE_ERROR; }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
            vcpu.vgic.lr[index] = virq;
            set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
            exception_t::EXCEPTION_NONE
        }
        _ => {
            unsafe { current_syscall_error._type = SEL4_ILLEGAL_OPERATION; }
            exception_t::EXCEPTION_SYSCALL_ERROR
        }
    }
}