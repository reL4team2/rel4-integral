use aarch64_cpu::registers::{
    CurrentEL, Readable, SPSel, CNTFRQ_EL0, CNTVCT_EL0, DAIF, MIDR_EL1, MPIDR_EL1, SP, TPIDRRO_EL0,
    TPIDR_EL0,
};
#[cfg(not(feature = "hypervisor"))]
use aarch64_cpu::registers::{
    ELR_EL1, ESR_EL1, FAR_EL1, MAIR_EL1, SCTLR_EL1, SPSR_EL1, TCR_EL1, TPIDR_EL1, TTBR0_EL1,
    TTBR1_EL1, VBAR_EL1,
};
#[cfg(feature = "hypervisor")]
use aarch64_cpu::registers::{
    ELR_EL2, ESR_EL2, FAR_EL2, HCR_EL2, MAIR_EL2, SCTLR_EL2, SPSR_EL2, TCR_EL2, TPIDR_EL2,
    TTBR0_EL2, VBAR_EL2, VTCR_EL2, VTTBR_EL2,
};

pub const PSCI_0_2_FN_BASE: u32 = 0x84000000;
pub const PSCI_0_2_64BIT: u32 = 0x40000000;
pub const PSCI_0_2_FN_CPU_SUSPEND: u32 = PSCI_0_2_FN_BASE + 1;
pub const PSCI_0_2_FN_CPU_OFF: u32 = PSCI_0_2_FN_BASE + 2;
pub const PSCI_0_2_FN_CPU_ON: u32 = PSCI_0_2_FN_BASE + 3;
pub const PSCI_0_2_FN_MIGRATE: u32 = PSCI_0_2_FN_BASE + 5;
pub const PSCI_0_2_FN_SYSTEM_OFF: u32 = PSCI_0_2_FN_BASE + 8;
pub const PSCI_0_2_FN_SYSTEM_RESET: u32 = PSCI_0_2_FN_BASE + 9;
pub const PSCI_0_2_FN64_CPU_SUSPEND: u32 = PSCI_0_2_FN_BASE + PSCI_0_2_64BIT + 1;
pub const PSCI_0_2_FN64_CPU_ON: u32 = PSCI_0_2_FN_BASE + PSCI_0_2_64BIT + 3;
pub const PSCI_0_2_FN64_MIGRATE: u32 = PSCI_0_2_FN_BASE + PSCI_0_2_64BIT + 5;

/// PSCI return values, inclusive of all PSCI versions.
#[derive(PartialEq, Debug)]
#[repr(i32)]
pub enum PsciError {
    NotSupported = -1,
    InvalidParams = -2,
    Denied = -3,
    AlreadyOn = -4,
    OnPending = -5,
    InternalFailure = -6,
    NotPresent = -7,
    Disabled = -8,
    InvalidAddress = -9,
}

impl From<i32> for PsciError {
    fn from(code: i32) -> PsciError {
        use PsciError::*;
        match code {
            -1 => NotSupported,
            -2 => InvalidParams,
            -3 => Denied,
            -4 => AlreadyOn,
            -5 => OnPending,
            -6 => InternalFailure,
            -7 => NotPresent,
            -8 => Disabled,
            -9 => InvalidAddress,
            _ => panic!("Unknown PSCI error code: {}", code),
        }
    }
}

/// psci "smc" method call
fn psci_smc_call(func: u32, arg0: usize, arg1: usize, arg2: usize) -> usize {
    let ret;
    unsafe {
        core::arch::asm!(
            "smc #0",
            inlateout("x0") func as usize => ret,
            in("x1") arg0,
            in("x2") arg1,
            in("x3") arg2,
        )
    }
    ret
}

fn psci_call(func: u32, arg0: usize, arg1: usize, arg2: usize) -> Result<(), PsciError> {
    let ret = psci_smc_call(func, arg0, arg1, arg2);

    match ret {
        0 => Ok(()),
        _ => Err(PsciError::from(ret as i32)),
    }
}

/// Dump the current CPU system/privileged registers to the log.
///
/// The general-purpose registers (x0..x30) are intentionally not read here:
/// they are snapshotted in assembly at the trap entry by `save_fault_registers`
/// (traps.S) and printed by `dump_fault_registers()` before halt() reaches this
/// point. Reading them again here would only show the clobbered halt/shutdown
/// call-frame values.
fn dump_register_state() {
    let sp = SP.get();
    let pc: usize;
    unsafe {
        core::arch::asm!("adr {pc}, .", pc = out(reg) pc);
    }

    log::info!("===== System register dump =====");
    log::info!("sp = {:#018x}  pc = {:#018x}", sp, pc);

    // System registers readable at every exception level.
    log::info!(
        "CurrentEL = {:#018x}  DAIF = {:#018x}  SPSel = {:#018x}",
        CurrentEL.get(),
        DAIF.get(),
        SPSel.get()
    );
    log::info!(
        "TPIDR_EL0 = {:#018x}  TPIDRRO_EL0 = {:#018x}",
        TPIDR_EL0.get(),
        TPIDRRO_EL0.get()
    );
    log::info!(
        "CNTVCT_EL0 = {:#018x}  CNTFRQ_EL0 = {:#018x}",
        CNTVCT_EL0.get(),
        CNTFRQ_EL0.get()
    );
    log::info!("MIDR_EL1 = {:#018x}  MPIDR_EL1 = {:#018x}", MIDR_EL1.get(), MPIDR_EL1.get());

    #[cfg(not(feature = "hypervisor"))]
    {
        log::info!("ELR_EL1 = {:#018x}  SPSR_EL1 = {:#018x}", ELR_EL1.get(), SPSR_EL1.get());
        log::info!("ESR_EL1 = {:#018x}  FAR_EL1 = {:#018x}", ESR_EL1.get(), FAR_EL1.get());
        log::info!("TTBR0_EL1 = {:#018x}  TTBR1_EL1 = {:#018x}", TTBR0_EL1.get(), TTBR1_EL1.get());
        log::info!("TPIDR_EL1 = {:#018x}", TPIDR_EL1.get());
        log::info!("SCTLR_EL1 = {:#018x}  MAIR_EL1 = {:#018x}", SCTLR_EL1.get(), MAIR_EL1.get());
        log::info!("TCR_EL1 = {:#018x}  VBAR_EL1 = {:#018x}", TCR_EL1.get(), VBAR_EL1.get());
    }

    #[cfg(feature = "hypervisor")]
    {
        log::info!("ELR_EL2 = {:#018x}  SPSR_EL2 = {:#018x}", ELR_EL2.get(), SPSR_EL2.get());
        log::info!("ESR_EL2 = {:#018x}  FAR_EL2 = {:#018x}", ESR_EL2.get(), FAR_EL2.get());
        log::info!("TTBR0_EL2 = {:#018x}  TPIDR_EL2 = {:#018x}", TTBR0_EL2.get(), TPIDR_EL2.get());
        log::info!("SCTLR_EL2 = {:#018x}  MAIR_EL2 = {:#018x}", SCTLR_EL2.get(), MAIR_EL2.get());
        log::info!("TCR_EL2 = {:#018x}  VBAR_EL2 = {:#018x}", TCR_EL2.get(), VBAR_EL2.get());
        log::info!(
            "HCR_EL2 = {:#018x}  VTTBR_EL2 = {:#018x}  VTCR_EL2 = {:#018x}",
            HCR_EL2.get(),
            VTTBR_EL2.get(),
            VTCR_EL2.get()
        );
    }
}

pub fn shutdown() -> ! {
    log::info!("Shutting down...");
    dump_register_state();
    psci_call(PSCI_0_2_FN_SYSTEM_OFF, 0, 0, 0).ok();
    panic!("It should shutdown!");
}
