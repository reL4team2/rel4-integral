//! 该模块在rust_sel4_pbf_parser模块生成的结构体的基础上，进行了一些功能的添加和封装。
//!
//! 记录在阅读代码段过程中用到的`cap`的特定字段含义：
//!
//! ```
//! untyped_cap:
//!  - capFreeIndex：从capPtr到可用的块的偏移，单位是2^seL4_MinUntypedBits大小的块数。如果seL4_MinUntypedBits是4，那么2^seL4_MinUntypedBits就是16字节。如果一个64字节的内存块已经分配了前32字节，则CapFreeIndex会存储2，因为已经使用了2个16字节的块。
//!  - capBlockSize：当前untyped块中剩余空间大小
//! endpoint_cap:
//!  - capEPBadge：当使用Mint方法创建一个新的endpoint_cap时，可以设置badge，用于表示派生关系，例如一个进程可以与多个进程通信，为了判断消息究竟来自哪个进程，就可以使用badge区分。
//! ```
//! Represent a capability, composed by two words. Different cap can contain different bit fields.

pub mod zombie;

use sel4_common::structures_gen::{cap, cap_null_cap, cap_tag};
use sel4_common::{sel4_config::*, MASK};

use crate::arch::arch_same_object_as;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct CNodeCapData {
    pub words: [usize; 1],
}

impl CNodeCapData {
    #[inline]
    pub fn new(data: usize) -> Self {
        CNodeCapData { words: [data] }
    }

    #[inline]
    pub fn get_guard(&self) -> usize {
        (self.words[0] & 0xffffffffffffffc0usize) >> 6
    }

    #[inline]
    pub fn get_guard_size(&self) -> usize {
        self.words[0] & 0x3fusize
    }
}

/// cap 的公用方法
pub trait cap_func {
    fn update_data(&self, preserve: bool, new_data: u64) -> Self;
    fn get_cap_size_bits(&self) -> usize;
    fn get_cap_is_physical(&self) -> bool;
    fn isArchCap(&self) -> bool;
}
pub trait cap_arch_func {
    fn get_cap_ptr(&self) -> usize;
    fn is_vtable_root(&self) -> bool;
    fn is_valid_native_root(&self) -> bool;
    fn is_valid_vtable_root(&self) -> bool;
}
impl cap_func for cap {
    fn update_data(&self, preserve: bool, new_data: u64) -> Self {
        if self.isArchCap() {
            return self.clone();
        }
        match self.get_tag() {
            cap_tag::cap_endpoint_cap => {
                if !preserve && cap::cap_endpoint_cap(self).get_capEPBadge() == 0 {
                    let mut new_cap = cap::cap_endpoint_cap(self).clone();
                    new_cap.set_capEPBadge(new_data);
                    new_cap.unsplay()
                } else {
                    cap_null_cap::new().unsplay()
                }
            }

            cap_tag::cap_notification_cap => {
                if !preserve && cap::cap_notification_cap(self).get_capNtfnBadge() == 0 {
                    let mut new_cap = cap::cap_notification_cap(self).clone();
                    new_cap.set_capNtfnBadge(new_data);
                    new_cap.unsplay()
                } else {
                    cap_null_cap::new().unsplay()
                }
            }

            cap_tag::cap_cnode_cap => {
                let w = CNodeCapData::new(new_data as usize);
                let guard_size = w.get_guard_size();
                if guard_size + cap::cap_cnode_cap(self).get_capCNodeRadix() as usize > wordBits {
                    return cap_null_cap::new().unsplay();
                }
                let guard = w.get_guard() & MASK!(guard_size);
                let mut new_cap = cap::cap_cnode_cap(self).clone();
                new_cap.set_capCNodeGuard(guard as u64);
                new_cap.set_capCNodeGuardSize(guard_size as u64);
                new_cap.unsplay()
            }
            _ => self.clone(),
        }
    }

    fn get_cap_size_bits(&self) -> usize {
        match self.get_tag() {
            cap_tag::cap_untyped_cap => cap::cap_untyped_cap(self).get_capBlockSize() as usize,
            cap_tag::cap_endpoint_cap => seL4_EndpointBits,
            cap_tag::cap_notification_cap => seL4_NotificationBits,
            cap_tag::cap_cnode_cap => {
                cap::cap_cnode_cap(self).get_capCNodeRadix() as usize + seL4_SlotBits
            }
            cap_tag::cap_page_table_cap => PT_SIZE_BITS,
            #[cfg(feature = "KERNEL_MCS")]
            cap_tag::cap_reply_cap => seL4_ReplyBits,
            #[cfg(not(feature = "KERNEL_MCS"))]
            cap_tag::cap_reply_cap => 0,
            _ => 0,
        }
    }

    fn get_cap_is_physical(&self) -> bool {
        matches!(
            self.get_tag(),
            cap_tag::cap_untyped_cap
                | cap_tag::cap_endpoint_cap
                | cap_tag::cap_notification_cap
                | cap_tag::cap_cnode_cap
                | cap_tag::cap_frame_cap
                | cap_tag::cap_asid_pool_cap
                | cap_tag::cap_page_table_cap
                | cap_tag::cap_zombie_cap
                | cap_tag::cap_thread_cap
        )
    }

    fn isArchCap(&self) -> bool {
        self.get_tag() as usize % 2 != 0
    }
}

/// 判断两个cap指向的内核对象是否是同一个内存区域
pub fn same_region_as(cap1: &cap, cap2: &cap) -> bool {
    match cap1.get_tag() {
        cap_tag::cap_untyped_cap => {
            if cap2.get_cap_is_physical() {
                let aBase = cap::cap_untyped_cap(cap1).get_capPtr() as usize;
                let bBase = cap2.get_cap_ptr();

                let aTop = aBase + MASK!(cap::cap_untyped_cap(cap1).get_capBlockSize());
                let bTop = bBase + MASK!(cap2.get_cap_size_bits());
                return (aBase <= bBase) && (bTop <= aTop) && (bBase <= bTop);
            }

            false
        }
        cap_tag::cap_endpoint_cap
        | cap_tag::cap_notification_cap
        | cap_tag::cap_page_table_cap
        | cap_tag::cap_asid_pool_cap
        | cap_tag::cap_thread_cap => {
            if cap2.get_tag() == cap1.get_tag() {
                return cap1.get_cap_ptr() == cap2.get_cap_ptr();
            }
            false
        }
        cap_tag::cap_asid_control_cap | cap_tag::cap_domain_cap => {
            if cap2.get_tag() == cap1.get_tag() {
                return true;
            }
            false
        }
        cap_tag::cap_cnode_cap => {
            if cap2.get_tag() == cap_tag::cap_cnode_cap {
                return (cap::cap_cnode_cap(cap1).get_capCNodePtr()
                    == cap::cap_cnode_cap(cap2).get_capCNodePtr())
                    && (cap::cap_cnode_cap(cap1).get_capCNodeRadix()
                        == cap::cap_cnode_cap(cap2).get_capCNodeRadix());
            }
            false
        }
        cap_tag::cap_irq_control_cap => {
            matches!(
                cap2.get_tag(),
                cap_tag::cap_irq_control_cap | cap_tag::cap_irq_handler_cap
            )
        }
        cap_tag::cap_irq_handler_cap => {
            if cap2.get_tag() == cap_tag::cap_irq_handler_cap {
                return cap::cap_irq_handler_cap(cap1).get_capIRQ()
                    == cap::cap_irq_handler_cap(cap2).get_capIRQ();
            }
            false
        }
        _ => false,
    }
}

/// Check whether two caps point to the same kernel object, if not,
///  whether two kernel objects use the same memory region.
///
/// A special case is that cap2 is a untyped_cap derived from cap1, in this case, cap1 will excute
/// setUntypedCapAsFull, so you can assume cap1 and cap2 are different.
pub fn same_object_as(cap1: &cap, cap2: &cap) -> bool {
    if cap1.get_tag() == cap_tag::cap_untyped_cap {
        return false;
    }
    if cap1.get_tag() == cap_tag::cap_irq_control_cap
        && cap2.get_tag() == cap_tag::cap_irq_handler_cap
    {
        return false;
    }
    if cap1.isArchCap() && cap2.isArchCap() {
        return arch_same_object_as(cap1, cap2);
    }
    same_region_as(cap1, cap2)
}

/// 判断一个`capability`是否是可撤销的
pub fn is_cap_revocable(derived_cap: &cap, src_cap: &cap) -> bool {
    if derived_cap.isArchCap() {
        return false;
    }

    match derived_cap.get_tag() {
        cap_tag::cap_endpoint_cap => {
            assert_eq!(src_cap.get_tag(), cap_tag::cap_endpoint_cap);
            cap::cap_endpoint_cap(derived_cap).get_capEPBadge()
                != cap::cap_endpoint_cap(src_cap).get_capEPBadge()
        }

        cap_tag::cap_notification_cap => {
            assert_eq!(src_cap.get_tag(), cap_tag::cap_notification_cap);
            cap::cap_notification_cap(derived_cap).get_capNtfnBadge()
                != cap::cap_notification_cap(src_cap).get_capNtfnBadge()
        }

        cap_tag::cap_irq_handler_cap => src_cap.get_tag() == cap_tag::cap_irq_control_cap,

        cap_tag::cap_untyped_cap => true,

        _ => false,
    }
}
