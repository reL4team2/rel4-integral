use core::mem::offset_of;

use aarch64_cpu::registers::{Readable, SPSR_EL1, TPIDR_EL0, TPIDRRO_EL0, Writeable};
use static_assertions::const_assert;

use crate::regs::ArchReg;

#[cfg(feature = "fpu")]
#[repr(C)]
#[derive(Debug, Clone)]
pub struct FPUState {
    /// 64 128-bit SIMD and floating-point registers
    vregs: [usize; 64],
    /// Floating-point Status Register
    /// <https://developer.arm.com/documentation/ddi0601/2025-06/AArch64-Registers/FPSR--Floating-point-Status-Register>
    fpsr: u32,
    /// Floating-point Control Register
    /// <https://developer.arm.com/documentation/ddi0601/2025-06/AArch64-Registers/FPCR--Floating-point-Control-Register>
    fpcr: u32,
}

#[cfg(feature = "fpu")]
impl Default for FPUState {
    fn default() -> Self {
        Self {
            vregs: [0; 64],
            fpsr: 0,
            fpcr: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct UserContext {
    /// General purpose registers
    pub gregs: [usize; 31],
    /// Stack Pointer
    pub sp: usize,
    /// Exception Link Register
    /// - EL2: <https://developer.arm.com/documentation/ddi0601/2025-06/AArch64-Registers/ELR-EL2--Exception-Link-Register--EL2->
    /// - EL1: <https://developer.arm.com/documentation/ddi0601/2025-06/AArch64-Registers/ELR-EL1--Exception-Link-Register--EL1->
    pub elr: usize,
    /// Saved Program Status Register
    /// - EL2: <https://developer.arm.com/documentation/ddi0601/2025-06/AArch64-Registers/SPSR-EL2--Saved-Program-Status-Register--EL2-
    /// - EL1: <https://developer.arm.com/documentation/ddi0601/2025-06/AArch64-Registers/SPSR-EL1--Saved-Program-Status-Register--EL1->
    pub spsr: u64,
    pub fault_ip: usize,
    /// Thread ID Register, Read/Write at EL0
    /// <https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/TPIDR-EL0--EL0-Read-Write-Software-Thread-ID-Register>
    pub tpidr_el0: u64,
    /// Thread ID Register, ReadOnly at EL0
    /// <https://developer.arm.com/documentation/ddi0601/2025-06/AArch64-Registers/TPIDRRO-EL0--EL0-Read-Only-Software-Thread-ID-Register>
    pub tpidrro_el0: u64,

    #[cfg(feature = "fpu")]
    /// Floating Point Unit state
    pub fpu: FPUState,
}

pub struct ArchTCB {
    pub ctx: UserContext,
}

impl Default for ArchTCB {
    fn default() -> Self {
        let mut uctx = UserContext::default();
        uctx.spsr = (SPSR_EL1::F::Masked + SPSR_EL1::A::Masked).value;
        Self { ctx: uctx }
    }
}

impl ArchTCB {
    /// Number of context registers
    pub const REGS_NUM: usize = 31 + 1 + 1 + 1 + 1 + 1;

    /// Configure the TCB as an idle thread
    ///
    /// # Arguments
    /// * `idle_thread_ip` - The instruction pointer of the idle thread.
    /// * `core` - The core number the idle thread will run on.
    pub fn config_idle_thread(&mut self, idle_thread_ip: usize, _core: usize) {
        self.ctx.elr = idle_thread_ip;
        #[cfg(feature = "hypervisor")]
        {
            self.ctx.spsr += aarch64_cpu::registers::SPSR_EL2::M::EL2h.value;
        }
        #[cfg(not(feature = "hypervisor"))]
        {
            self.ctx.spsr += aarch64_cpu::registers::SPSR_EL1::M::EL1h.value;
        }
    }

    /// Save TLS(Thread local Storage) registers
    #[inline]
    pub fn save_thread_local(&mut self) {
        self.ctx.tpidr_el0 = TPIDR_EL0.get();
        self.ctx.tpidrro_el0 = TPIDRRO_EL0.get();
    }

    /// Load TLS(Thread local Storage) registers
    #[inline]
    pub fn load_thread_local(&mut self) {
        TPIDR_EL0.set(self.ctx.tpidr_el0);
        TPIDRRO_EL0.set(self.ctx.tpidrro_el0);
    }

    #[cfg(feature = "fpu")]
    #[inline]
    pub fn fpu_state_ptr(&mut self) -> *const FPUState {
        &self.ctx.fpu as *const FPUState
    }

    /// Set the register of the TCB
    /// # Arguments
    /// * `reg` - The register index.
    /// * `w` - The value to set.
    #[inline]
    pub fn set_register(&mut self, _reg: ArchReg, _w: usize) {
        unimplemented!("set_register @ aarch64");
        // self.registers[reg.to_index()] = w;
    }

    /// Get the register value of the TCB
    /// # Arguments
    /// * `reg` - The register index.
    /// # Returns
    /// The value of the register.
    #[inline]
    pub fn get_register(&self, _reg: ArchReg) -> usize {
        // self.registers[reg.to_index()]
        unimplemented!("get_register @ aarch64")
    }
}

// Ensure the offsets match the seL4 expectations
const_assert!(offset_of!(UserContext, sp) == 31 * core::mem::size_of::<usize>());
