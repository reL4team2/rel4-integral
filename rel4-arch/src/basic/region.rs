use crate::basic::{PAddr, PPtr, VPtr};

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

impl Region {
    pub const fn new(start: PPtr, end: PPtr) -> Self {
        Self { start, end }
    }

    /// Create a empty region area.
    ///
    /// start is null(0) and end is null(0)
    pub const fn empty() -> Self {
        Self {
            start: PPtr::new(0),
            end: PPtr::new(0),
        }
    }

    /// Check if the region area is zero
    pub const fn is_empty(&self) -> bool {
        self.start.raw() == self.end.raw()
    }

    /// Convert [PPtr] Region [Region] to [PAddr] region [PRegion]
    pub const fn to_pregion(&self) -> PRegion {
        PRegion::new(self.start.to_paddr(), self.end.to_paddr())
    }
}

impl From<Region> for PRegion {
    fn from(value: Region) -> Self {
        value.to_pregion()
    }
}

impl PRegion {
    pub const fn new(start: PAddr, end: PAddr) -> Self {
        Self { start, end }
    }

    /// Create a empty region area.
    ///
    /// start is null(0) and end is null(0)
    pub const fn empty() -> Self {
        Self {
            start: PAddr::new(0),
            end: PAddr::new(0),
        }
    }

    /// Check if the region area is zero
    pub const fn is_empty(&self) -> bool {
        self.start.raw() == self.end.raw()
    }

    /// Convert [PAddr] region [PRegion] to [PPtr] Region [Region]
    pub const fn to_region(&self) -> Region {
        Region::new(self.start.to_pptr(), self.end.to_pptr())
    }
}

impl From<PRegion> for Region {
    fn from(value: PRegion) -> Self {
        value.to_region()
    }
}

impl VRegion {
    pub const fn new(start: VPtr, end: VPtr) -> Self {
        Self { start, end }
    }
}
