//! This file contains the implementation of the `ObjectType` enum and its associated methods.
//! The `ObjectType` enum represents the different types of objects in the system.
//! It provides methods to retrieve the size of an object, the frame type of an object,
//! convert a usize value to an `ObjectType`, and check if an object type is architecture-specific.

use crate::arch::ObjectType;

use super::sel4_config::*;

#[cfg(target_arch = "riscv64")]
pub const OBJECT_TYPE_COUNT: usize = ObjectType::PageTableObject as usize + 1;
// FIXED: Need to add 1 to cover all possible object types
#[cfg(any(target_arch = "aarch64", test))]
pub const OBJECT_TYPE_COUNT: usize = ObjectType::seL4_ARM_PageTableObject as usize + 1;
#[cfg(not(feature = "kernel_mcs"))]
pub const NON_ARCH_OBJECT_TYPE_COUNT: usize = ObjectType::CapTableObject as usize + 1;
#[cfg(feature = "kernel_mcs")]
pub const NON_ARCH_OBJECT_TYPE_COUNT: usize = ObjectType::ReplyObject as usize + 1;

impl ObjectType {
    /// Returns the size of the object based on its type.
    ///
    /// # Arguments
    ///
    /// * `user_object_size` - The size of the user object.
    ///
    /// # Returns
    ///
    /// The size of the object.
    pub fn get_object_size(&self, user_object_size: usize) -> usize {
        if (*self) as usize >= NON_ARCH_OBJECT_TYPE_COUNT {
            return self.arch_get_object_size();
        }
        match self {
            ObjectType::UnytpedObject => user_object_size,
            ObjectType::TCBObject => seL4_TCBBits,
            ObjectType::EndpointObject => seL4_EndpointBits,
            ObjectType::NotificationObject => seL4_NotificationBits,
            ObjectType::CapTableObject => seL4_SlotBits + user_object_size,
            #[cfg(feature = "kernel_mcs")]
            ObjectType::SchedContextObject => user_object_size,
            #[cfg(feature = "kernel_mcs")]
            ObjectType::ReplyObject => seL4_ReplyBits,
            _ => panic!("unsupported cap type:{}", (*self) as usize),
        }
    }

    /// Converts a usize value to an ObjectType.
    ///
    /// # Arguments
    ///
    /// * `value` - The usize value to convert.
    ///
    /// # Returns
    ///
    /// An Option containing the converted ObjectType, or None if the value is out of range.
    pub fn from_usize(value: usize) -> Option<Self> {
        if value >= OBJECT_TYPE_COUNT {
            return None;
        }
        unsafe { Some(core::mem::transmute::<u8, ObjectType>(value as u8)) }
    }
}
