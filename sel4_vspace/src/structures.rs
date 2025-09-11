use core::fmt::Debug;

use sel4_common::{structures::exception_t, structures_gen::lookup_fault};

use crate::PTE;

/// 进程对应的asid所属的类型
pub type asid_t = usize;

#[cfg(target_arch = "riscv64")]
#[repr(C)]
#[derive(Clone)]
pub struct findVSpaceForASID_ret {
    pub status: exception_t,
    pub vspace_root: Option<*mut PTE>,
    pub lookup_fault: Option<lookup_fault>,
}
#[cfg(target_arch = "aarch64")]
#[repr(C)]
#[derive(Clone)]
pub struct findVSpaceForASID_ret {
    pub status: exception_t,
    pub vspace_root: Option<*mut PTE>,
    pub lookup_fault: Option<lookup_fault>,
}

/// 进行系统调用时，应用程序向内核传递信息的消息格式
///
/// vm_attributes_t is a message type. When program pass message to kernel , it uses vm_attributes_t.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct vm_attributes_t(pub(crate) usize);

impl vm_attributes_t {
    pub fn new(value: usize) -> Self {
        Self(value)
    }

    pub fn from_word(w: usize) -> Self {
        Self::new(w)
    }

    pub fn get_execute_never(&self) -> usize {
        self.0 & 0x1usize
    }

    pub fn set_execute_never(&mut self, v64: usize) {
        self.0 &= !0x1usize;
        self.0 |= (v64 << 0) & 0x1usize;
    }

    pub fn get_page_cacheable(&self) -> usize {
        self.0 & 0x1
    }
}
