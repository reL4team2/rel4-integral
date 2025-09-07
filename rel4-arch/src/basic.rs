use core::ops::{Add, AddAssign, Sub};

use rel4_utils::impl_multi;

/// Pointer to User-Virtual Memory
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VPtr(usize);

/// Pointer to Physical Memory
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PAddr(usize);

/// Pointer to Kernel-Virtual Memory
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

    /// Check if the value is zero
    pub const fn is_null(&self) -> bool {
        self.0 == 0
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

impl PPtr {
    /// Get the const pointer for the [PPtr]
    pub fn get_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    /// Get the mutable pointer for the [PPtr]
    pub fn get_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }

    /// Get reference for the [PPtr]
    ///
    /// Should ensure the value of [PPtr] is valid
    pub fn get_ref<T>(&self) -> &'static T {
        unsafe { &*self.get_ptr::<T>() }
    }

    /// Get mutable reference for the [PPtr]
    ///
    /// Should ensure the value of [PPtr] is valid
    pub fn get_mut_ref<T>(&self) -> &'static mut T {
        unsafe { &mut *self.get_mut_ptr::<T>() }
    }
}

impl Add<usize> for PPtr {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl AddAssign<usize> for PPtr {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs
    }
}

impl Add<usize> for PAddr {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0.add(rhs))
    }
}

impl Sub<usize> for PAddr {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0.sub(rhs))
    }
}

impl Region {
    pub const fn empty() -> Self {
        Self {
            start: PPtr::new(0),
            end: PPtr::new(0),
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.start.raw() == self.end.raw()
    }
}

impl PRegion {
    pub const fn new(start: PAddr, end: PAddr) -> Self {
        Self { start, end }
    }

    pub const fn empty() -> Self {
        Self {
            start: PAddr::new(0),
            end: PAddr::new(0),
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.start.raw() == self.end.raw()
    }
}
