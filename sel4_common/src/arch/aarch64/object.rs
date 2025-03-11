use crate::sel4_config::{
    seL4_PGDBits, seL4_PUDBits, seL4_PageDirBits, seL4_PageTableBits, seL4_VSpaceBits,
    ARMHugePageBits, ARMLargePageBits, ARMSmallPageBits, ARM_Huge_Page, ARM_Large_Page,
    ARM_Small_Page,
};
#[cfg(not(feature = "KERNEL_MCS"))]
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

#[cfg(feature = "KERNEL_MCS")]
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
            Self::seL4_ARM_SmallPageObject => ARMSmallPageBits,
            Self::seL4_ARM_LargePageObject => ARMLargePageBits,
            Self::seL4_ARM_HugePageObject => ARMHugePageBits,
            Self::seL4_ARM_PageTableObject => seL4_PageTableBits,
            Self::seL4_ARM_VSpaceObject => seL4_VSpaceBits,
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
            ObjectType::seL4_ARM_SmallPageObject => ARM_Small_Page,
            ObjectType::seL4_ARM_LargePageObject => ARM_Large_Page,
            ObjectType::seL4_ARM_HugePageObject => ARM_Huge_Page,
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
