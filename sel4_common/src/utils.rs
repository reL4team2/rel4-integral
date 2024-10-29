//! Utility functions and macros.
use core::slice;

use crate::sel4_config::*;

#[macro_export]
/// Return fill the given number of bits with 1.
macro_rules! MASK {
    ($e:expr) => {
        {
             (1usize << $e) - 1usize
        }
    }
}

#[macro_export]
/// Calculate the floor of the given number.
macro_rules! ROUND_DOWN {
    ($n:expr,$b:expr) => {{
        ((($n) >> ($b)) << ($b))
    }};
}

#[macro_export]
/// Calculate the ceil of the given number.
macro_rules! ROUND_UP {
    ($n:expr,$b:expr) => {{
        ((((($n) - 1usize) >> ($b)) + 1usize) << ($b))
    }};
}

#[macro_export]
/// Judge whether the given number is aligned to the given number of bits.
macro_rules! IS_ALIGNED {
    ($n:expr,$b:expr) => {{
        $n & MASK!($b) == 0
    }};
}

#[macro_export]
/// Calculate 1 << n for given n.
macro_rules! BIT {
    ($e:expr) => {
        {
            1usize<<$e
        }
    }
}

/// Get the global variable.
/// WARN: But on smp, need to becareful to use this macro.
/// TODO: Write macro ffi_set or other functions to set the global variable
#[cfg(not(feature = "SMP"))]
pub macro global_read($name: ident) {
    unsafe { $name }
}

#[cfg(not(feature = "SMP"))]
pub macro global_ops($expr: expr) {
    unsafe { $expr }
}

#[inline]
pub fn MAX_FREE_INDEX(bits: usize) -> usize {
    BIT!(bits - seL4_MinUntypedBits)
}

#[inline]
pub fn convert_ref_type_to_usize<T>(addr: &mut T) -> usize {
    addr as *mut T as usize
}

#[inline]
pub fn convert_to_type_ref<T>(addr: usize) -> &'static T {
    assert_ne!(addr, 0);
    unsafe { &*(addr as *mut T) }
}

#[inline]
pub fn convert_to_mut_type_ref<T>(addr: usize) -> &'static mut T {
    assert_ne!(addr, 0);
    unsafe { &mut *(addr as *mut T) }
}

#[inline]
pub fn convert_to_mut_type_ptr<T>(addr: usize) -> *mut T {
    assert_ne!(addr, 0);
    addr as *mut T
}

#[inline]
pub fn convert_to_mut_type_ref_unsafe<T>(addr: usize) -> &'static mut T {
    unsafe { &mut *(addr as *mut T) }
}

#[inline]
pub fn convert_to_option_type_ref<T>(addr: usize) -> Option<&'static T> {
    if addr == 0 {
        return None;
    }
    Some(convert_to_type_ref::<T>(addr))
}

#[inline]
pub fn convert_to_option_mut_type_ref<T>(addr: usize) -> Option<&'static mut T> {
    if addr == 0 {
        return None;
    }
    Some(convert_to_mut_type_ref::<T>(addr))
}

/// Get the slice through passed arguments
///
/// addr: The address of the slice
/// len: The length of the slice
#[inline]
pub fn convert_to_mut_slice<T>(addr: usize, len: usize) -> &'static mut [T] {
    unsafe { slice::from_raw_parts_mut(addr as _, len) }
}

/// Convert a ptr to a reference
#[inline]
pub fn ptr_to_ref<T>(ptr: *const T) -> &'static T {
    unsafe { ptr.as_ref().unwrap() }
}

/// Convert a ptr to a mutable reference
#[inline]
pub fn ptr_to_mut<T>(ptr: *mut T) -> &'static mut T {
    unsafe { ptr.as_mut().unwrap() }
}

#[inline]
pub fn cpu_id() -> usize {
    #[cfg(feature = "ENABLE_SMP")]
    {
        use crate::smp::get_currenct_cpu_index;
        get_currenct_cpu_index()
    }
    #[cfg(not(feature = "ENABLE_SMP"))]
    {
        0
    }
}

#[no_mangle]
#[inline]
pub fn pageBitsForSize(page_size: usize) -> usize {
    match page_size {
        RISCV_4K_Page => RISCVPageBits,
        RISCV_Mega_Page => RISCVMegaPageBits,
        RISCV_Giga_Page => RISCVGigaPageBits,
        _ => panic!("Invalid page size!"),
    }
}
