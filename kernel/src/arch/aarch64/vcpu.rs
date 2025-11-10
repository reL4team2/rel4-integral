use core::intrinsics::likely;

use aarch64_cpu::{
    asm::barrier,
    registers::{Readable, Writeable, CPACR_EL1, HCR_EL2, ID_AA64MMFR0_EL1, SCTLR_EL1, VTCR_EL2},
};
use sel4_task::tcb_t;

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
    // HVM(26) | TTLB(25) | TPU(24) | TPC(23) | TSW(22) | TAC(21)
    | bits!(26, 25, 24, 23, 22, 21);

// TWE(14) | TWI(13)
const HCR_VCPU: u64 = HCR_COMMON | bits!(14, 13);

static mut ARM_HS_CUR_VCPU: *mut VCpu = 0 as _;
static mut ARM_HS_VCPU_ACTIVE: bool = false;
// const HCR_NATIVE: HCR_EL2 = HCR_EL2::

/// Armv8 init vcpu in boot stage
///
/// ID_AA64MMFR0_EL1: <https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64MMFR0-EL1--AArch64-Memory-Model-Feature-Register-0>
/// VTCR_EL2: <https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/VTCR-EL2--Virtualization-Translation-Control-Register>
pub fn armv_vcpu_boot_init() {
    if ID_AA64MMFR0_EL1.matches_all(ID_AA64MMFR0_EL1::TGran4::NotSupported) {
        panic!("Processor doesn't support 4KB");
    }
    VTCR_EL2.write(
        // CONFIG_ARM_PA_SIZE_BITS_40
        VTCR_EL2::T0SZ.val(24)
            + VTCR_EL2::PS::PA_40B_1TB
            + VTCR_EL2::SL0.val(1)
        // END CONFIG_ARM_PA_SIZE_BITS_40
            + VTCR_EL2::IRGN0::NormalWBRAWA
            + VTCR_EL2::ORGN0::NormalWBRAWA
            + VTCR_EL2::SH0::Inner
            + VTCR_EL2::TG0::Granule4KB,
    );
    barrier::dsb(barrier::SY);
}

pub fn vcpu_boot_init() {
    armv_vcpu_boot_init();
    // TODO: initialize VGIC
    log::warn!("initialize vgic is not implemented");
    vcpu_disable(0 as _);
    unsafe {
        ARM_HS_CUR_VCPU = 0 as _;
        ARM_HS_VCPU_ACTIVE = false;
    }
}

pub fn vcpu_disable(vcpu: *mut VCpu) {
    barrier::dsb(barrier::SY);

    if likely(!vcpu.is_null()) {
        let vcpu = unsafe { &mut *vcpu };
        // hcr =
        log::warn!("save vgic hcr");
        vcpu.regs.sctlr = SCTLR_EL1.get();
        vcpu.regs.cpacr = CPACR_EL1.get();
    }

    log::warn!("set_gic_vcpu_ctrl_hcr to Turn Off VGic");
    barrier::isb(barrier::SY);

    SCTLR_EL1.set(SCTLR_DEFAULT);
    barrier::isb(barrier::SY);
    HCR_EL2.set(HCR_NATIVE);

    barrier::isb(barrier::SY);
    if likely(!vcpu.is_null()) {
        log::warn!("save_virt_timer vcpu");
        #[cfg(feature = "enable_smp")]
        log::warn!("convert core_irq to irqt");
        mask_interrupt(true, INTERRUPT_VTIMER_EVENT);
    }
}

pub fn vcpu_enable(vcpu: &VCpu) {
    SCTLR_EL1.set(vcpu.regs.sctlr);
    HCR_EL2.set(HCR_VCPU);
    barrier::isb(barrier::SY);
    log::warn!("set_gic_vcpu_ctrl_hcr to vcpu's gic hcr");
    CPACR_EL1.set(vcpu.regs.cpacr);
    log::warn!("restore virt timer");
}

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
    tcb: *mut tcb_t,
    vgic: GICVCpuIface,
    regs: VCpuRegSet,
    vppi_masked: [bool; 1],
    /* word_t vcpu_padding; */
    /* vTimer is 8-bytes wide and has the same 8-byte alignment requirement.
     * If the sum of n_VPPIEventIRQ and seL4_VCPUReg_Num is odd, we do not need
     * extra padding. If the sum is even we do. It currently is odd, so the extra
     * padding above is unnecessary for the struct to remain packed on 32 bit
     * platforms.
     */
    vtimer: VTimer,
}
