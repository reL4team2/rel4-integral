use rel4_utils::impl_multi;

/// Pointer to User-Virtual Memory
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct VPtr(usize);

/// Pointer to Physical Memory
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct PAddr(usize);

/// Pointer to Kernel-Virtual Memory
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct PPtr(usize);

/// Pointer to Capability Node
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct CPtr(usize);

/// Kernel Virtual Memory Region
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Region {
    pub start: PPtr,
    pub end: PPtr,
}

/// Physical Memory Region
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PRegion {
    pub start: PAddr,
    pub end: PAddr,
}

/// User-Virtual Memory Region
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct VRegion {
    pub start: VPtr,
    pub end: VPtr,
}

impl_multi!(VPtr, PAddr, PPtr, CPtr {
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    /// Get the raw value [usize]
    pub const fn raw(&self) -> usize {
        self.0
    }

    /// Aligns the value down to the nearest multiple of 2^`bits`.
    /// Effectively clears the lower `bits` of the value.
    pub const fn align_down(&self, bits: usize) -> Self {
        Self((self.0 >> bits) << bits)
    }

    /// Aligns the value up to the nearest multiple of 2^`bits`.
    /// If already aligned, the value is unchanged.
    pub const fn align_up(&self, bits: usize) -> Self {
        let align_size = bit!(bits);
        Self::new(self.0.div_ceil(align_size) * align_size)
    }

    /// Check if the value is aligned on a 2^`bits` boundary.
    pub const fn aligned(&self, bits: usize) -> bool {
        self.0 & bit!(bits) == 0
    }
});
