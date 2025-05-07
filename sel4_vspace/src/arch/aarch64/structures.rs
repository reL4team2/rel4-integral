use core::ops::{Deref, DerefMut};

use crate::{vm_attributes_t, PTE};
use sel4_common::{
    sel4_config::ASID_LOW_BITS, structures_gen::asid_map, utils::convert_to_mut_type_ref, BIT,
};

use super::machine::mair_types;

impl vm_attributes_t {
    pub fn get_arm_execute_never(&self) -> bool {
        if (self.0 & 0x4) != 0 {
            true
        } else {
            false
        }
    }

    pub fn get_arm_page_cachable(&self) -> bool {
        if (self.0 & 0x1) != 0 {
            true
        } else {
            false
        }
    }

    pub fn get_attr_index(&self) -> mair_types {
        if self.get_arm_page_cachable() {
            return mair_types::NORMAL;
        }

        mair_types::DEVICE_nGnRnE
    }
}

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
#[repr(C)]
#[derive(Clone, Debug)]
pub struct asid_pool_t([asid_map; BIT!(ASID_LOW_BITS)]);

/// Dereference for asid_pool_t.
///
/// Allow directly accessing values
impl DerefMut for asid_pool_t {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Dereference for asid_pool_t.
///
/// Allow directly accessing values
impl Deref for asid_pool_t {
    type Target = [asid_map; BIT!(ASID_LOW_BITS)];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Get the slice of the page_table items
///
/// Addr should be virtual address.
pub(super) fn asid_pool_from_addr(addr: usize) -> &'static mut asid_pool_t {
    // ASID Pool's len is BIT!(ASID_LOW_BITS)
    // convert_to_mut_slice::<>(addr, BIT!(ASID_LOW_BITS))
    assert_ne!(addr, 0);
    convert_to_mut_type_ref(addr)
}
