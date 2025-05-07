//! This module contains the structures used in the seL4 microkernel.
//! For example, the `seL4_IPCBuffer` struct represents the IPC buffer used for inter-process communication in seL4.
//! The `exception_t` enum represents the different types of exceptions in the system.
use super::sel4_config::*;

const PADDING_VALUE: isize = isize::MAX - 1;
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Represents the different types of exceptions in the system.
pub enum exception_t {
    EXCEPTION_NONE,
    EXCEPTION_FAULT,
    EXCEPTION_LOOKUP_FAULT,
    EXCEPTION_SYSCALL_ERROR,
    EXCEPTION_PREEMTED,
    padding = PADDING_VALUE,
}

#[repr(C)]
#[derive(Copy, Clone)]
/// Represents the IPC buffer used for inter-process communication in seL4.
pub struct seL4_IPCBuffer {
    /// The tag field of the IPC message.
    pub tag: usize,
    /// The message payload of the IPC message.
    pub msg: [usize; SEL4_MSG_MAX_LENGTH],
    /// User-defined data associated with the IPC message.
    pub userData: usize,
    /// Array of capabilities or badges associated with the IPC message.
    pub caps_or_badges: [usize; SEL4_MSG_MAX_EXTRA_CAPS],
    /// The capability node where the IPC message is received.
    pub receiveCNode: usize,
    /// The index within the capability node where the IPC message is received.
    pub receiveIndex: usize,
    /// The depth of the capability node where the IPC message is received.
    pub receiveDepth: usize,
}

impl seL4_IPCBuffer {
    pub fn get_extra_cptr(&self, i: usize) -> usize {
        self.caps_or_badges[i]
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct p_region_t {
    pub start: usize,
    pub end: usize,
}

use crate::arch::config::PPTR_BASE;
use core::{
    ffi::CStr,
    fmt::{Debug, Display},
};
pub type pptr_t = usize;

#[derive(Copy, Clone)]
pub struct kernel_frame_t {
    pub paddr: paddr_t,
    pub pptr: pptr_t,
    pub armExecuteNever: isize,
    pub userAvailable: isize,
}

#[repr(C)]
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct paddr_t(pub usize);
impl From<usize> for paddr_t {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl paddr_t {
    #[inline]
    pub fn addr(&self) -> usize {
        self.0
    }

    #[inline]
    pub fn get_ptr<T>(&self) -> *const T {
        (self.0 | PPTR_BASE) as *const T
    }

    #[inline]
    pub const fn get_mut_ptr<T>(&self) -> *mut T {
        (self.0 | PPTR_BASE) as *mut T
    }

    #[inline]
    pub fn slice_with_len<T>(&self, len: usize) -> &'static [T] {
        unsafe { core::slice::from_raw_parts(self.get_ptr(), len) }
    }

    #[inline]
    pub fn slice_mut_with_len<T>(&self, len: usize) -> &'static mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.get_mut_ptr(), len) }
    }

    #[inline]
    pub fn get_cstr(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.get_ptr::<i8>()) }
    }
}

impl Debug for paddr_t {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{:#x}", self.0))
    }
}

impl Display for paddr_t {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{:#x}", self.0))
    }
}

cfg_if::cfg_if! {
    if #[cfg(all(feature = "enable_smp", target_arch = "aarch64"))] {
        #[repr(C)]
        pub struct irq_t {
            pub irq: usize,
            pub core: usize,
        }
    } else {
        pub type irq_t = usize;
    }
}
