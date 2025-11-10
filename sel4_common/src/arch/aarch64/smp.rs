use aarch64_cpu::registers;
use aarch64_cpu::registers::Readable;

use core::arch::asm;

#[inline]
pub fn cpu_index_to_id(index: usize) -> usize {
    return bit!(index);
}

/// Read the current CPU index from TPIDR_EL1
///
/// - TPIDR_EL1: <https://developer.arm.com/documentation/ddi0601/2025-06/AArch64-Registers/TPIDR-EL1--EL1-Software-Thread-ID-Register>
/// - TPIDR_EL2: <https://developer.arm.com/documentation/ddi0601/2025-06/AArch64-Registers/TPIDR-EL2--EL2-Software-Thread-ID-Register>
#[inline]
pub fn get_current_cpu_index() -> usize {
    #[cfg(not(feature = "hypervisor"))]
    {
        registers::TPIDR_EL1.get() as usize & 0xfff
    }
    #[cfg(feature = "hypervisor")]
    {
        registers::TPIDR_EL2.get() as usize & 0xfff
    }
}
