use aarch64_cpu::registers::Readable;

/// Get the value of the FAR register.
#[inline]
pub fn get_far() -> usize {
    #[cfg(feature = "hypervisor")]
    {
        aarch64_cpu::registers::FAR_EL2.get() as _
    }
    #[cfg(not(feature = "hypervisor"))]
    {
        aarch64_cpu::registers::FAR_EL1.get() as _
    }
}

#[inline]
pub fn get_esr() -> usize {
    #[cfg(feature = "hypervisor")]
    {
        aarch64_cpu::registers::ESR_EL2.get() as _
    }
    #[cfg(not(feature = "hypervisor"))]
    {
        aarch64_cpu::registers::ESR_EL1.get() as _
    }
}
