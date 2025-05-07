use crate::sel4_config::{
    ARM_HUGE_PAGE, ARM_HUGE_PAGE_BITS, ARM_LARGE_PAGE, ARM_LARGE_PAGE_BITS, ARM_SMALL_PAGE,
    ARM_SMALL_PAGE_BITS, SEL4_PAGE_DIR_BITS, SEL4_PAGE_TABLE_BITS, SEL4_PGD_BITS, SEL4_PUD_BITS,
    SEL4_VSPACE_BITS,
};
#[cfg(not(feature = "kernel_mcs"))]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
/// Represents the type of an object.
pub enum ObjectType {
    UnytpedObject = 0,
    TCBObject = 1,
    EndpointObject = 2,
    NotificationObject = 3,
    CapTableObject = 4,
    // aarch64 relevant object
    seL4_ARM_HugePageObject = 5,
    seL4_ARM_VSpaceObject = 6,
    seL4_ARM_SmallPageObject = 7,
    seL4_ARM_LargePageObject = 8,
    seL4_ARM_PageTableObject = 9,
}

#[cfg(feature = "kernel_mcs")]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum ObjectType {
    UnytpedObject = 0,
    TCBObject = 1,
    EndpointObject = 2,
    NotificationObject = 3,
    CapTableObject = 4,
    SchedContextObject = 5,
    ReplyObject = 6,
    // aarch64 relevant object
    seL4_ARM_HugePageObject = 7,
    seL4_ARM_VSpaceObject = 8,
    seL4_ARM_SmallPageObject = 9,
    seL4_ARM_LargePageObject = 10,
    seL4_ARM_PageTableObject = 11,
}

impl ObjectType {
    pub fn arch_get_object_size(&self) -> usize {
        match self {
            Self::seL4_ARM_SmallPageObject => ARM_SMALL_PAGE_BITS,
            Self::seL4_ARM_LargePageObject => ARM_LARGE_PAGE_BITS,
            Self::seL4_ARM_HugePageObject => ARM_HUGE_PAGE_BITS,
            Self::seL4_ARM_PageTableObject => SEL4_PAGE_TABLE_BITS,
            Self::seL4_ARM_VSpaceObject => SEL4_VSPACE_BITS,
            _ => panic!("unsupported object type:{}", *self as usize),
        }
    }

    /// Returns the frame type of the object.
    ///
    /// # Returns
    ///
    /// The frame type of the object.
    pub fn get_frame_type(&self) -> usize {
        match self {
            ObjectType::seL4_ARM_SmallPageObject => ARM_SMALL_PAGE,
            ObjectType::seL4_ARM_LargePageObject => ARM_LARGE_PAGE,
            ObjectType::seL4_ARM_HugePageObject => ARM_HUGE_PAGE,
            _ => {
                panic!("Invalid frame type: {:?}", self);
            }
        }
    }
    /// Checks if the object type is an architecture-specific type.
    ///
    /// # Returns
    ///
    /// true if the object type is an architecture-specific type, false otherwise.
    pub fn is_arch_type(self) -> bool {
        matches!(
            self,
            Self::seL4_ARM_HugePageObject
                | Self::seL4_ARM_SmallPageObject
                | Self::seL4_ARM_LargePageObject
        )
    }
}
