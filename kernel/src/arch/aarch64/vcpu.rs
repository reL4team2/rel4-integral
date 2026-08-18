use core::intrinsics::likely;

use aarch64_cpu::{
    asm::barrier,
    registers::{Readable, Writeable, HCR_EL2, ID_AA64MMFR0_EL1, SCTLR_EL1, VTCR_EL2},
};
use sel4_common::{
    arch::MessageLabel,
    platform::INTERRUPT_VTIMER_EVENT,
    structures::{exception_t, seL4_IPCBuffer},
};
use sel4_cspace::interface::cte_t;
use sel4_task::{get_currenct_thread, set_thread_state, tcb_t, ThreadState};

use super::arm_gic::gic_v2::{
    get_gic_vcpu_ctrl_apr, get_gic_vcpu_ctrl_eisr0, get_gic_vcpu_ctrl_eisr1, get_gic_vcpu_ctrl_hcr,
    get_gic_vcpu_ctrl_lr, get_gic_vcpu_ctrl_misr, get_gic_vcpu_ctrl_vmcr, gic_vcpu_num_list_regs,
    set_gic_vcpu_ctrl_apr, set_gic_vcpu_ctrl_hcr, set_gic_vcpu_ctrl_lr, set_gic_vcpu_ctrl_vmcr,
};
use crate::interrupt::mask_interrupt;

const VMCS_SIZE: usize = 4096;
const IOBITMAP_SIZE: usize = 8192;

/// TODO: GIC_V2 is 16, and GIC_V3 is 64
const GIC_VCPU_MAX_NUM_LR: usize = 16;

const SCTLR_DEFAULT: u64 = 0xc5187c;
const ACTLR_DEFAULT: u64 = 0x40;

/// GICH_HCR enable bit.
const VGIC_HCR_EN: u32 = 1;

/// seL4 VCPU register indices. Must match `seL4_VCPUReg` in
/// `sel4_arch_include/aarch64/sel4/sel4_arch/constants.h`.
const VCPU_REG_SCTLR: usize = 0;
const VCPU_REG_CPACR: usize = 1;
const VCPU_REG_TTBR0: usize = 2;
const VCPU_REG_TTBR1: usize = 3;
const VCPU_REG_TCR: usize = 4;
const VCPU_REG_MAIR: usize = 5;
const VCPU_REG_AMAIR: usize = 6;
const VCPU_REG_CIDR: usize = 7;
const VCPU_REG_ACTLR: usize = 8;
const VCPU_REG_AFSR0: usize = 9;
const VCPU_REG_AFSR1: usize = 10;
const VCPU_REG_ESR: usize = 11;
const VCPU_REG_FAR: usize = 12;
const VCPU_REG_PAR: usize = 13;
const VCPU_REG_VBAR: usize = 14;
const VCPU_REG_TPIDR_EL1: usize = 15;
const VCPU_REG_VMPIDR_EL2: usize = 16;
const VCPU_REG_SP_EL1: usize = 17;
const VCPU_REG_ELR_EL1: usize = 18;
const VCPU_REG_SPSR_EL1: usize = 19;
const VCPU_REG_CNTV_CTL: usize = 20;
const VCPU_REG_CNTV_CVAL: usize = 21;
const VCPU_REG_CNTVOFF: usize = 22;
const VCPU_REG_CNTKCTL_EL1: usize = 23;
const VCPU_REG_NUM: usize = 24;
const VCPU_REG_SAVE_RANGE_START: usize = VCPU_REG_TTBR0;
const VCPU_REG_SAVE_RANGE_END: usize = VCPU_REG_SPSR_EL1;

/// VPPI event indices (matches `enum VPPIEventIRQ` in the C kernel).
const VPPI_EVENT_IRQ_VTIMER: usize = 0;
const VPPI_EVENT_IRQ_INVALID: usize = 1;

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
    let vtcr_val = { VTCR_EL2::T0SZ.val(24) + VTCR_EL2::PS::PA_40B_1TB + VTCR_EL2::SL0.val(1) };
    #[cfg(not(all(feature = "pa_40bit", feature = "hypervisor")))]
    let vtcr_val = { VTCR_EL2::T0SZ.val(20) + VTCR_EL2::PS::PA_44B_16TB + VTCR_EL2::SL0.val(2) };
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
    // Set MAIR_EL2 for stage-2 translation. Stage-2 uses 16 x 4-bit attributes
    // (Attr0-Attr15); attribute 15 (S2_NORMAL) = Normal Inner/Outer WB-WA
    // (0b1111). The elfloader programs MAIR_EL2 with stage-1's 8-bit layout,
    // leaving Attr15 as Device — so stage-2 Normal pages fault on real hardware.
    /*
    const MAIR_EL2_VALUE: u64 = (0x0u64 << 0)   // Attr0:  Device-nGnRnE
        | (0x1u64 << 4)                          // Attr1:  Device-nGnRE
        | (0x2u64 << 8)                          // Attr2:  Device-nGRE
        | (0x3u64 << 12)                         // Attr3:  Device-GRE
        | (0xfu64 << 60); // Attr15: S2_NORMAL (Normal WB-WA/WB-WA)
    */
    const MAIR_EL2_VALUE: u64 = (0x00u64 << 0)  // Attr0 (8-bit): Device-nGnRnE  -> S2 Attr0=0
        | (0x04u64 << 8)                         // Attr1 (8-bit): Device-nGnRE
        | (0x0cu64 << 16)                        // Attr2 (8-bit): Device-GRE
        | (0x44u64 << 24)                        // Attr3 (8-bit): Normal-NC
        | (0xffu64 << 32)                        // Attr4 (8-bit): Normal WB-WA (kernel NORMAL=4)
        | (0xaau64 << 40)                        // Attr5 (8-bit): Normal-WT
        | (0xf0u64 << 56); // Attr7 high nibble=0xf -> stage-2 Attr15=S2_NORMAL
    unsafe { core::arch::asm!("msr mair_el2, {}", in(reg) MAIR_EL2_VALUE) };
    barrier::isb(barrier::SY);
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
        unsafe {
            GIC_NUM_LIST_REGS = GIC_VCPU_MAX_NUM_LR;
        }
    }
    vcpu_disable(0 as _);
    unsafe {
        ARM_HS_CUR_VCPU = 0;
        ARM_HS_VCPU_ACTIVE = false;
    }
}

// ---------------------------------------------------------------------------
// VCPU register read/write helpers (mirrors C kernel vcpu.h / vcpu.c)
// ---------------------------------------------------------------------------

macro_rules! vcpu_mrs {
    ($reg:literal) => {{
        let v: usize;
        unsafe {
            core::arch::asm!(concat!("mrs {}, ", $reg), out(reg) v);
        }
        v
    }};
}

macro_rules! vcpu_msr {
    ($reg:literal, $v:expr) => {{
        unsafe {
            core::arch::asm!(concat!("msr ", $reg, ", {}"), in(reg) $v);
        }
    }};
}

fn vcpu_hw_read_reg(reg_index: usize) -> usize {
    match reg_index {
        VCPU_REG_SCTLR => vcpu_mrs!("sctlr_el1"),
        VCPU_REG_CPACR => vcpu_mrs!("cpacr_el1"),
        VCPU_REG_TTBR0 => vcpu_mrs!("ttbr0_el1"),
        VCPU_REG_TTBR1 => vcpu_mrs!("ttbr1_el1"),
        VCPU_REG_TCR => vcpu_mrs!("tcr_el1"),
        VCPU_REG_MAIR => vcpu_mrs!("mair_el1"),
        VCPU_REG_AMAIR => vcpu_mrs!("amair_el1"),
        VCPU_REG_CIDR => vcpu_mrs!("contextidr_el1"),
        VCPU_REG_ACTLR => vcpu_mrs!("actlr_el1"),
        VCPU_REG_AFSR0 => vcpu_mrs!("afsr0_el1"),
        VCPU_REG_AFSR1 => vcpu_mrs!("afsr1_el1"),
        VCPU_REG_ESR => vcpu_mrs!("esr_el1"),
        VCPU_REG_FAR => vcpu_mrs!("far_el1"),
        VCPU_REG_PAR => vcpu_mrs!("par_el1"),
        VCPU_REG_VBAR => vcpu_mrs!("vbar_el1"),
        VCPU_REG_TPIDR_EL1 => vcpu_mrs!("tpidr_el1"),
        VCPU_REG_VMPIDR_EL2 => vcpu_mrs!("vmpidr_el2"),
        VCPU_REG_SP_EL1 => vcpu_mrs!("sp_el1"),
        VCPU_REG_ELR_EL1 => vcpu_mrs!("elr_el1"),
        VCPU_REG_SPSR_EL1 => vcpu_mrs!("spsr_el1"),
        VCPU_REG_CNTV_CTL => vcpu_mrs!("cntv_ctl_el0"),
        VCPU_REG_CNTV_CVAL => vcpu_mrs!("cntv_cval_el0"),
        VCPU_REG_CNTVOFF => vcpu_mrs!("cntvoff_el2"),
        VCPU_REG_CNTKCTL_EL1 => vcpu_mrs!("cntkctl_el1"),
        _ => panic!("ARM/HYP: Invalid register index"),
    }
}

fn vcpu_hw_write_reg(reg_index: usize, reg: usize) {
    match reg_index {
        VCPU_REG_SCTLR => vcpu_msr!("sctlr_el1", reg),
        VCPU_REG_CPACR => vcpu_msr!("cpacr_el1", reg),
        VCPU_REG_TTBR0 => vcpu_msr!("ttbr0_el1", reg),
        VCPU_REG_TTBR1 => vcpu_msr!("ttbr1_el1", reg),
        VCPU_REG_TCR => vcpu_msr!("tcr_el1", reg),
        VCPU_REG_MAIR => vcpu_msr!("mair_el1", reg),
        VCPU_REG_AMAIR => vcpu_msr!("amair_el1", reg),
        VCPU_REG_CIDR => vcpu_msr!("contextidr_el1", reg),
        VCPU_REG_ACTLR => vcpu_msr!("actlr_el1", reg),
        VCPU_REG_AFSR0 => vcpu_msr!("afsr0_el1", reg),
        VCPU_REG_AFSR1 => vcpu_msr!("afsr1_el1", reg),
        VCPU_REG_ESR => vcpu_msr!("esr_el1", reg),
        VCPU_REG_FAR => vcpu_msr!("far_el1", reg),
        VCPU_REG_PAR => vcpu_msr!("par_el1", reg),
        VCPU_REG_VBAR => vcpu_msr!("vbar_el1", reg),
        VCPU_REG_TPIDR_EL1 => vcpu_msr!("tpidr_el1", reg),
        VCPU_REG_VMPIDR_EL2 => vcpu_msr!("vmpidr_el2", reg),
        VCPU_REG_SP_EL1 => vcpu_msr!("sp_el1", reg),
        VCPU_REG_ELR_EL1 => vcpu_msr!("elr_el1", reg),
        VCPU_REG_SPSR_EL1 => vcpu_msr!("spsr_el1", reg),
        VCPU_REG_CNTV_CTL => vcpu_msr!("cntv_ctl_el0", reg),
        VCPU_REG_CNTV_CVAL => vcpu_msr!("cntv_cval_el0", reg),
        VCPU_REG_CNTVOFF => vcpu_msr!("cntvoff_el2", reg),
        VCPU_REG_CNTKCTL_EL1 => vcpu_msr!("cntkctl_el1", reg),
        _ => panic!("ARM/HYP: Invalid register index"),
    }
}

fn vcpu_reg_saved_when_disabled(field: usize) -> bool {
    matches!(
        field,
        VCPU_REG_SCTLR
            | VCPU_REG_CNTV_CTL
            | VCPU_REG_CNTV_CVAL
            | VCPU_REG_CNTVOFF
            | VCPU_REG_CNTKCTL_EL1
            | VCPU_REG_CPACR
    )
}

fn vcpu_save_reg(vcpu: &mut VCpu, reg: usize) {
    assert!(reg < VCPU_REG_NUM);
    vcpu.regs[reg] = vcpu_hw_read_reg(reg);
}

fn vcpu_save_reg_range(vcpu: &mut VCpu, start: usize, end: usize) {
    for i in start..=end {
        vcpu_save_reg(vcpu, i);
    }
}

fn vcpu_restore_reg(vcpu: &VCpu, reg: usize) {
    assert!(reg < VCPU_REG_NUM);
    vcpu_hw_write_reg(reg, vcpu.regs[reg]);
}

fn vcpu_restore_reg_range(vcpu: &VCpu, start: usize, end: usize) {
    for i in start..=end {
        vcpu_restore_reg(vcpu, i);
    }
}

fn read_vcpu_reg(vcpu: &VCpu, field: usize) -> usize {
    if likely(unsafe { ARM_HS_CUR_VCPU } == vcpu as *const VCpu as usize) {
        if vcpu_reg_saved_when_disabled(field) && !unsafe { ARM_HS_VCPU_ACTIVE } {
            vcpu.regs[field]
        } else {
            vcpu_hw_read_reg(field)
        }
    } else {
        vcpu.regs[field]
    }
}

fn write_vcpu_reg(vcpu: &mut VCpu, field: usize, value: usize) {
    if likely(unsafe { ARM_HS_CUR_VCPU } == vcpu as *mut VCpu as usize) {
        if vcpu_reg_saved_when_disabled(field) && !unsafe { ARM_HS_VCPU_ACTIVE } {
            vcpu.regs[field] = value;
        } else {
            vcpu_hw_write_reg(field, value);
        }
    } else {
        vcpu.regs[field] = value;
    }
}

/// Initialize a freshly retyped VCPU object (mirrors C `vcpu_init`).
pub fn vcpu_init(vcpu: &mut VCpu) {
    // armv_vcpu_init(): initial SCTLR for a VCPU thread.
    vcpu.regs[VCPU_REG_SCTLR] = SCTLR_DEFAULT as usize;
    // GICH VCPU interface control.
    vcpu.vgic.hcr = VGIC_HCR_EN;
    // Virtual timer interface.
    vcpu.vtimer.last_pcount = 0;
}

// ---------------------------------------------------------------------------
// VCPU enable / disable
// ---------------------------------------------------------------------------

pub fn vcpu_disable(vcpu: usize) {
    barrier::dsb(barrier::SY);

    if likely(vcpu != 0) {
        let vcpu = unsafe { &mut *(vcpu as *mut VCpu) };
        // Save VGIC HCR (soon to be off)
        vcpu.vgic.hcr = get_gic_vcpu_ctrl_hcr();
        vcpu_save_reg(vcpu, VCPU_REG_SCTLR);
        vcpu_save_reg(vcpu, VCPU_REG_CPACR);
        barrier::isb(barrier::SY);
    }

    // Turn off the VGIC
    set_gic_vcpu_ctrl_hcr(0);
    barrier::isb(barrier::SY);

    SCTLR_EL1.set(SCTLR_DEFAULT);
    barrier::isb(barrier::SY);
    // In native mode (no VCPU), enable FP/SIMD BEFORE TGE=1,
    // because afterwards CPACR_EL1 access redirects to CPTR_EL2.
    unsafe {
        crate::arch::aarch64::fpu::native_enable_fpu();
    }
    HCR_EL2.set(HCR_NATIVE);
    if likely(vcpu != 0) {
        #[cfg(feature = "enable_smp")]
        mask_interrupt(true, INTERRUPT_VTIMER_EVENT);
    }
}

pub fn vcpu_enable(vcpu: &VCpu) {
    vcpu_restore_reg(vcpu, VCPU_REG_SCTLR);
    HCR_EL2.set(HCR_VCPU);
    barrier::isb(barrier::SY);
    // Restore VGIC HCR
    set_gic_vcpu_ctrl_hcr(vcpu.vgic.hcr);
    // Restore CPACR_EL1 with FPEN=0b11 and update FPU state cache.
    unsafe {
        crate::arch::aarch64::fpu::vcpu_restore_fpu(vcpu.regs[VCPU_REG_CPACR] as u64);
    }
}

// ---------------------------------------------------------------------------
// VCPU save / restore
// ---------------------------------------------------------------------------

/// Save the currently active VCPU's state.
fn vcpu_save(vcpu: &mut VCpu, active: bool) {
    barrier::dsb(barrier::SY);
    if active {
        vcpu_save_reg(vcpu, VCPU_REG_SCTLR);
        vcpu.vgic.hcr = get_gic_vcpu_ctrl_hcr();
    }
    // Always save GIC control state regardless of active
    vcpu.vgic.vmcr = get_gic_vcpu_ctrl_vmcr();
    vcpu.vgic.apr = get_gic_vcpu_ctrl_apr();
    let lr_num = unsafe { GIC_NUM_LIST_REGS };
    for i in 0..lr_num {
        vcpu.vgic.lr[i] = get_gic_vcpu_ctrl_lr(i) as usize;
    }
    // armv_vcpu_save(): CPACR (when active) plus the whole save/restore range.
    if active {
        vcpu_save_reg(vcpu, VCPU_REG_CPACR);
    }
    vcpu_save_reg_range(vcpu, VCPU_REG_SAVE_RANGE_START, VCPU_REG_SAVE_RANGE_END);
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

    // Restore the saved register range.
    vcpu_restore_reg_range(vcpu, VCPU_REG_SAVE_RANGE_START, VCPU_REG_SAVE_RANGE_END);

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
        dissociate_vcpu_tcb(unsafe { &mut *(tcb.tcbArch.tcbVCPU as *mut VCpu) }, tcb);
    }
    if vcpu.tcb != 0 {
        let old_tcb = unsafe { &mut *(vcpu.tcb as *mut tcb_t) };
        dissociate_vcpu_tcb(vcpu, old_tcb);
    }
    tcb.tcbArch.tcbVCPU = vcpu as *mut VCpu as usize;
    vcpu.tcb = tcb as *mut tcb_t as usize;

    // If associating the current thread's VCPU, switch to it immediately.
    if tcb.is_current() {
        vcpu_switch(vcpu as *mut VCpu as usize);
    }
}

/// Handle a VCPU fault (HSR from Stage-2 translation fault or VCPU exit).
/// Attempts armv-level handling (FPU traps, HVC), then sends VCPUFault to VMM.
/// Returns true if the fault was fully handled (FPU, HVC) and the caller
/// should NOT schedule+activate — just restore_user_context.
/// Returns false if the caller should proceed with schedule+activate.
pub unsafe fn handle_vcpu_fault(hsr: usize) -> bool {
    use sel4_common::ffi::current_fault;
    use sel4_task::get_currenct_thread;

    use crate::syscall::handle_fault;

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
            unsafe {
                crate::arch::aarch64::fpu::handle_fpu_fault();
            }
            return true; // handled, caller should NOT schedule
        }
        // Log when FPU trap is ignored (fpu_enabled=true or unexpected EC)
        static mut FPU_IGNORE_COUNT: usize = 0;
        unsafe {
            FPU_IGNORE_COUNT += 1;
        }
        if unsafe { FPU_IGNORE_COUNT } & 0xFF == 0 {
            log::debug!(
                "[FPU_IGNORE] cnt={} esr_ec={:#x} fpu_enabled={}",
                unsafe { FPU_IGNORE_COUNT },
                esr_ec,
                fpu_enabled
            );
        }
    }

    // Try armv-level handling first
    if hsr == 0x2000000 {
        // UNKNOWN_FAULT: likely HVC from guest
        crate::arch::aarch64::exception::handleUserLevelFault(
            crate::arch::aarch64::instruction::get_esr(),
            0,
        );
        return true; // handled, caller should NOT schedule
    }

    // Otherwise send VCPUFault to user-level VMM
    unsafe {
        current_fault =
            sel4_common::structures_gen::seL4_Fault_VCPUFault::new(hsr as u64).unsplay();
    }
    handle_fault(get_currenct_thread());
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
    use sel4_common::ffi::current_fault;
    use sel4_task::{activateThread, get_currenct_thread, schedule};

    use crate::syscall::handle_fault;

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
                current_fault =
                    sel4_common::structures_gen::seL4_Fault_VGICMaintenance::new(irq_idx as u64, 1)
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
    use sel4_task::get_currenct_thread;

    use crate::syscall::handle_fault;

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
        current_fault =
            sel4_common::structures_gen::seL4_Fault_VPPIEvent::new(irq as u64).unsplay();
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
    pub regs: [usize; VCPU_REG_NUM],
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
    call: bool,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    use sel4_common::structures_gen::cap;

    let vcpu_ptr = cap::cap_vcpu_cap(&slot.capability).get_capVCPUPtr() as usize;
    let vcpu = unsafe { &mut *(vcpu_ptr as *mut VCpu) };

    match label {
        MessageLabel::ARMVCPUSetTCB => decode_vcpu_set_tcb(vcpu),
        MessageLabel::ARMVCPUReadReg => decode_vcpu_read_reg(vcpu, length, call, buffer),
        MessageLabel::ARMVCPUWriteReg => decode_vcpu_write_reg(vcpu, length, buffer),
        MessageLabel::ARMVCPUInjectIRQ => decode_vcpu_inject_irq(vcpu, length, buffer),
        MessageLabel::ARMVCPUAckVPPI => decode_vcpu_ack_vppi(vcpu, length, buffer),
        _ => {
            use sel4_common::sel4_config::SEL4_ILLEGAL_OPERATION;

            use crate::kernel::boot::current_syscall_error;
            unsafe {
                current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
            }
            exception_t::EXCEPTION_SYSCALL_ERROR
        },
    }
}

#[cfg(feature = "hypervisor")]
fn decode_vcpu_set_tcb(vcpu: &mut VCpu) -> exception_t {
    use sel4_common::{
        sel4_config::{SEL4_ILLEGAL_OPERATION, SEL4_TRUNCATED_MESSAGE},
        structures_gen::{cap, cap_tag},
    };

    use crate::kernel::boot::{current_syscall_error, get_extra_cap_by_index};

    let tcb_slot = match get_extra_cap_by_index(0) {
        Some(slot) => slot,
        None => {
            unsafe {
                current_syscall_error._type = SEL4_TRUNCATED_MESSAGE;
            }
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        },
    };

    if tcb_slot.capability.get_tag() != cap_tag::cap_thread_cap {
        unsafe {
            current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let tcb_ptr = cap::cap_thread_cap(&tcb_slot.capability).get_capTCBPtr() as usize;
    let tcb = unsafe { &mut *(tcb_ptr as *mut tcb_t) };
    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    invoke_vcpu_set_tcb(vcpu, tcb)
}

#[cfg(feature = "hypervisor")]
fn invoke_vcpu_set_tcb(vcpu: &mut VCpu, tcb: &mut tcb_t) -> exception_t {
    associate_vcpu_tcb(vcpu, tcb);
    exception_t::EXCEPTION_NONE
}

#[cfg(feature = "hypervisor")]
fn decode_vcpu_read_reg(
    vcpu: &mut VCpu,
    length: usize,
    call: bool,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    use sel4_common::sel4_config::{SEL4_INVALID_ARGUMENT, SEL4_TRUNCATED_MESSAGE};

    use crate::{kernel::boot::current_syscall_error, syscall::get_syscall_arg};

    if length < 1 {
        unsafe {
            current_syscall_error._type = SEL4_TRUNCATED_MESSAGE;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let field = get_syscall_arg(0, buffer);
    if field >= VCPU_REG_NUM {
        unsafe {
            current_syscall_error._type = SEL4_INVALID_ARGUMENT;
            current_syscall_error.invalidArgumentNumber = 1;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    invoke_vcpu_read_reg(vcpu, field, call)
}

#[cfg(feature = "hypervisor")]
fn invoke_vcpu_read_reg(vcpu: &VCpu, field: usize, call: bool) -> exception_t {
    use sel4_common::{
        arch::ArchReg, message_info::seL4_MessageInfo_func, shared_types_bf_gen::seL4_MessageInfo,
    };

    let thread = get_currenct_thread();
    let value = read_vcpu_reg(vcpu, field);
    if call {
        thread.tcbArch.set_register(ArchReg::Badge, 0);
        let length = thread.set_mr(0, value) as u64;
        thread
            .tcbArch
            .set_register(ArchReg::MsgInfo, seL4_MessageInfo::new(0, 0, 0, length).to_word());
    }
    set_thread_state(thread, ThreadState::ThreadStateRunning);
    exception_t::EXCEPTION_NONE
}

#[cfg(feature = "hypervisor")]
fn decode_vcpu_write_reg(vcpu: &mut VCpu, length: usize, buffer: &seL4_IPCBuffer) -> exception_t {
    use sel4_common::sel4_config::{SEL4_INVALID_ARGUMENT, SEL4_TRUNCATED_MESSAGE};

    use crate::{kernel::boot::current_syscall_error, syscall::get_syscall_arg};

    if length < 2 {
        unsafe {
            current_syscall_error._type = SEL4_TRUNCATED_MESSAGE;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let field = get_syscall_arg(0, buffer);
    let value = get_syscall_arg(1, buffer);
    if field >= VCPU_REG_NUM {
        unsafe {
            current_syscall_error._type = SEL4_INVALID_ARGUMENT;
            current_syscall_error.invalidArgumentNumber = 1;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    invoke_vcpu_write_reg(vcpu, field, value)
}

#[cfg(feature = "hypervisor")]
fn invoke_vcpu_write_reg(vcpu: &mut VCpu, field: usize, value: usize) -> exception_t {
    write_vcpu_reg(vcpu, field, value);
    exception_t::EXCEPTION_NONE
}

#[cfg(feature = "hypervisor")]
fn decode_vcpu_inject_irq(vcpu: &mut VCpu, length: usize, buffer: &seL4_IPCBuffer) -> exception_t {
    use sel4_common::{
        sel4_config::{SEL4_DELETE_FIRST, SEL4_RANGE_ERROR, SEL4_TRUNCATED_MESSAGE},
        structures_gen::{virq_tag, virq_virq_pending},
    };

    use crate::{kernel::boot::current_syscall_error, syscall::get_syscall_arg};

    if length < 1 {
        unsafe {
            current_syscall_error._type = SEL4_TRUNCATED_MESSAGE;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let mr0 = get_syscall_arg(0, buffer);
    let vid = mr0 & 0xffff;
    let priority = (mr0 >> 16) & 0xff;
    let group = (mr0 >> 24) & 0xff;
    let index = (mr0 >> 32) & 0xff;

    if vid > (1 << 10) - 1 {
        unsafe {
            current_syscall_error._type = SEL4_RANGE_ERROR;
            current_syscall_error.rangeErrorMin = 0;
            current_syscall_error.rangeErrorMax = (1 << 10) - 1;
            current_syscall_error.invalidArgumentNumber = 1;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    if priority > 31 {
        unsafe {
            current_syscall_error._type = SEL4_RANGE_ERROR;
            current_syscall_error.rangeErrorMin = 0;
            current_syscall_error.rangeErrorMax = 31;
            current_syscall_error.invalidArgumentNumber = 2;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    if group > 1 {
        unsafe {
            current_syscall_error._type = SEL4_RANGE_ERROR;
            current_syscall_error.rangeErrorMin = 0;
            current_syscall_error.rangeErrorMax = 1;
            current_syscall_error.invalidArgumentNumber = 3;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let num_list_regs = unsafe { GIC_NUM_LIST_REGS };
    if index >= num_list_regs {
        unsafe {
            current_syscall_error._type = SEL4_RANGE_ERROR;
            current_syscall_error.rangeErrorMin = 0;
            current_syscall_error.rangeErrorMax = num_list_regs - 1;
            current_syscall_error.invalidArgumentNumber = 4;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    // LR index is in use.
    if ((vcpu.vgic.lr[index] >> 28) & 0x3) == virq_tag::virq_virq_active as usize {
        unsafe {
            current_syscall_error._type = SEL4_DELETE_FIRST;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let virq = virq_virq_pending::new(group as u64, priority as u64, 1, vid as u64);
    let virq_raw = virq.0.arr[0] as usize;

    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    invoke_vcpu_inject_irq(vcpu, index, virq_raw)
}

#[cfg(feature = "hypervisor")]
fn invoke_vcpu_inject_irq(vcpu: &mut VCpu, index: usize, virq: usize) -> exception_t {
    if likely(unsafe { ARM_HS_CUR_VCPU } == vcpu as *mut VCpu as usize) {
        set_gic_vcpu_ctrl_lr(index, virq as u32);
    } else {
        vcpu.vgic.lr[index] = virq;
    }
    exception_t::EXCEPTION_NONE
}

#[cfg(feature = "hypervisor")]
fn decode_vcpu_ack_vppi(vcpu: &mut VCpu, length: usize, buffer: &seL4_IPCBuffer) -> exception_t {
    use sel4_common::{
        platform::{NUM_PPI, NUM_PPI_MINUS_ONE},
        sel4_config::{SEL4_INVALID_ARGUMENT, SEL4_RANGE_ERROR, SEL4_TRUNCATED_MESSAGE},
    };

    use crate::{kernel::boot::current_syscall_error, syscall::get_syscall_arg};

    if length < 1 {
        unsafe {
            current_syscall_error._type = SEL4_TRUNCATED_MESSAGE;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let irq_w = get_syscall_arg(0, buffer);
    // Arch_checkIRQ: VPPI events are PPIs (0 .. NUM_PPI).
    if irq_w >= NUM_PPI {
        unsafe {
            current_syscall_error._type = SEL4_RANGE_ERROR;
            current_syscall_error.rangeErrorMin = 0;
            current_syscall_error.rangeErrorMax = NUM_PPI_MINUS_ONE;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let vppi = irq_vppi_event_index(irq_w);
    if vppi == VPPI_EVENT_IRQ_INVALID {
        unsafe {
            current_syscall_error._type = SEL4_INVALID_ARGUMENT;
            current_syscall_error.invalidArgumentNumber = 0;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    invoke_vcpu_ack_vppi(vcpu, vppi)
}

#[cfg(feature = "hypervisor")]
fn invoke_vcpu_ack_vppi(vcpu: &mut VCpu, vppi: usize) -> exception_t {
    vcpu.vppi_masked[vppi] = false;
    exception_t::EXCEPTION_NONE
}

#[cfg(feature = "hypervisor")]
fn irq_vppi_event_index(irq: usize) -> usize {
    if irq == INTERRUPT_VTIMER_EVENT {
        VPPI_EVENT_IRQ_VTIMER
    } else {
        VPPI_EVENT_IRQ_INVALID
    }
}
