#[cfg(target_arch = "riscv64")]
pub mod riscv64;
use core::ops::Range;

#[cfg(target_arch = "riscv64")]
pub use riscv64::*;

#[cfg(any(target_arch = "aarch64", test))]
mod aarch64;
#[cfg(any(target_arch = "aarch64", test))]
pub use aarch64::*;

/// ArchTCB Common part
impl ArchTCB {
    /// Set the register of the TCB
    /// # Arguments
    /// * `reg` - The register index.
    /// * `w` - The value to set.
    #[inline]
    pub fn set_register(&mut self, reg: ArchReg, w: usize) {
        self.registers[reg.to_index()] = w;
    }

    /// Get the register value of the TCB
    /// # Arguments
    /// * `reg` - The register index.
    /// # Returns
    /// The value of the register.
    #[inline]
    pub const fn get_register(&self, reg: ArchReg) -> usize {
        self.registers[reg.to_index()]
    }

    /// Copy the value of a range from source TCB to destination TCB
    #[inline]
    pub fn copy_range(&mut self, source: &Self, range: Range<usize>) {
        self.registers[range.clone()].copy_from_slice(&source.registers[range]);
    }

    /// Get the raw pointer of the TCB
    ///
    /// Used in the `restore_user_context`
    #[inline]
    pub fn raw_ptr(&self) -> usize {
        self as *const ArchTCB as usize
    }
}

/// Arch Register Shared part
/// If not shared. Just Write in the [arch] module.
#[repr(usize)]
#[derive(Debug, Clone)]
pub enum ArchReg {
    /// Generic registers
    TlsBase,
    Cap,
    Badge,
    MsgInfo,
    FAULT_IP,
    NEXT_IP,
    /// Message Registers Msg(offset)
    Msg(usize),
    /// Frame Registers Frame(Offset)
    Frame(usize),
    /// GPRegisters GP(offset)
    GP(usize),
    /// Fault Message Reg, (id, index)
    FaultMessage(usize, usize),
    #[cfg(feature = "kernel_mcs")]
    Reply,
    #[cfg(feature = "kernel_mcs")]
    nbsRecvDest,
    #[cfg(target_arch = "riscv64")]
    SSTATUS,
}

#[cfg(feature = "kernel_mcs")]
use crate::platform::time_def::ticks_t;

#[cfg(feature = "kernel_mcs")]
pub fn get_kernel_wcet_ticks() -> ticks_t {
    us_to_ticks(get_kernel_wcet_us())
}

#[cfg(any(target_arch = "aarch64", test))]
pub type FPUState = aarch64::arch_tcb::FPUState;
