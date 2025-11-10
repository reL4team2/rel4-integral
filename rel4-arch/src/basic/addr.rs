use core::ops::{Add, AddAssign, BitAnd, BitOr, Sub, SubAssign};

use rel4_utils::impl_multi;

#[cfg(target_arch = "aarch64")]
use crate::aarch64::config::PPTR_BASE_OFFSET;

#[cfg(target_arch = "riscv64")]
use crate::riscv64::config::PPTR_BASE_OFFSET;

/// Pointer to User-Virtual Memory
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct VPtr(usize);

/// Pointer to Physical Memory
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct PAddr(usize);

/// Pointer to Kernel-Virtual Memory
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct PPtr(usize);

/// Pointer to Capability Node
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct CPtr(usize);

impl_multi!(VPtr, PAddr, PPtr, CPtr {
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    /// Get the raw value [usize]
    pub const fn raw(&self) -> usize {
        self.0
    }

    /// Get the raw value [u64]
    pub const fn as_u64(&self) -> u64 {
        self.0 as _
    }

    /// Create a Pointer which address is 0
    pub const fn null() -> Self {
        Self(0)
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

    /// Get mutable reference for the [PPtr]
    ///
    /// Should ensure the value of [PPtr] is valid
    pub fn get_mut_slice<const LEN: usize, T>(&self) -> &'static mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.get_mut_ptr(), LEN) }
    }

    /// Trying to get mutable reference for the [PPtr]
    ///
    /// Return [Option::None] if the address is invalid
    #[inline]
    pub fn try_get_mut_ref<T>(&self) -> Option<&'static mut T> {
        (!self.is_null()).then(|| self.get_mut_ref())
    }

    /// Convert [PPtr](Kernel-Virtual Pointer) to [PAddr](Physical Memory Address)
    pub const fn to_paddr(&self) -> PAddr {
        PAddr(self.0 - PPTR_BASE_OFFSET)
    }
}

impl From<PPtr> for PAddr {
    fn from(value: PPtr) -> Self {
        value.to_paddr()
    }
}

impl PAddr {
    /// Convert [PAddr](Physical Memory Address) to [PPtr](Kernel-Virtual Pointer)
    pub const fn to_pptr(&self) -> PPtr {
        PPtr(self.0 + PPTR_BASE_OFFSET)
    }
}

impl From<PAddr> for PPtr {
    fn from(value: PAddr) -> Self {
        value.to_pptr()
    }
}

macro_rules! impl_num_traits {
    ($($name:ident),*) => {
        $(
            impl Add<usize> for $name {
                type Output = Self;

                fn add(self, rhs: usize) -> Self::Output {
                    Self(self.0 + rhs)
                }
            }

            impl AddAssign<usize> for $name {
                fn add_assign(&mut self, rhs: usize) {
                    self.0.add_assign(rhs)
                }
            }

            impl Sub<usize> for $name {
                type Output = Self;

                fn sub(self, rhs: usize) -> Self::Output {
                    Self(self.0.sub(rhs))
                }
            }

            impl SubAssign<usize> for $name {
                fn sub_assign(&mut self, rhs: usize) {
                    self.0.sub_assign(rhs);
                }
            }

            impl BitAnd<usize> for $name {
                type Output = Self;

                fn bitand(self, rhs: usize) -> Self::Output {
                    Self(self.0.bitand(rhs))
                }
            }

            impl BitOr<usize> for $name {
                type Output = Self;

                fn bitor(self, rhs: usize) -> Self::Output {
                    Self(self.0.bitor(rhs))
                }
            }
        )*
    };
}

impl_num_traits!(PPtr, PAddr, VPtr);
