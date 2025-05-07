use sel4_common::{sel4_config::ASID_LOW_BITS, utils::convert_to_option_mut_type_ref, BIT};

use crate::{pptr_t, PTE};

///lookup_pt_slot函数的返回值，
/// `ptSlot`：找到的虚地址对应的`pte`的存放槽
/// `ptBitsLeft`:找到叶子节点时，虚地址剩余未被索引的位置
#[repr(C)]
#[derive(Copy, Clone)]
pub struct lookupPTSlot_ret_t {
    pub ptSlot: *mut PTE,
    pub ptBitsLeft: usize,
}

/// 用于存放`asid`对应的根页表基址，是一个`usize`的数组，其中`asid`按低`ASID_LOW_BITS`位进行索引
#[derive(Copy, Clone)]
pub struct asid_pool_t {
    pub array: [*mut PTE; BIT!(ASID_LOW_BITS)],
}

/// `asid pool`相关操作
impl asid_pool_t {
    #[inline]
    pub fn get_ptr(&self) -> pptr_t {
        self as *const Self as pptr_t
    }

    #[inline]
    pub fn get_vspace_by_index(&mut self, index: usize) -> Option<&'static mut PTE> {
        convert_to_option_mut_type_ref::<PTE>(self.array[index] as usize)
    }

    #[inline]
    pub fn set_vspace_by_index(&mut self, index: usize, vspace_ptr: pptr_t) {
        // assert!(index < BIT!(ASID_LOW_BITS));
        self.array[index] = vspace_ptr as *mut PTE;
    }
}
