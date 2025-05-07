use crate::sel4_config::{
    RISCV_4K_PAGE, RISCV_GIGA_PAGE, RISCV_MEGA_PAGE, SEL4_HUGE_PAGE_BITS, SEL4_LARGE_PAGE_BITS,
    SEL4_PAGE_BITS,
};

/// Represents the type of an object.
#[cfg(not(feature = "kernel_mcs"))]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum ObjectType {
    UnytpedObject = 0,
    TCBObject = 1,
    EndpointObject = 2,
    NotificationObject = 3,
    CapTableObject = 4,
    // RISCV relevant object
    GigaPageObject = 5,
    NormalPageObject = 6,
    MegaPageObject = 7,
    PageTableObject = 8,
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
    // RISCV relevant object
    GigaPageObject = 7,
    NormalPageObject = 8,
    MegaPageObject = 9,
    PageTableObject = 10,
}

impl ObjectType {
    pub fn arch_get_object_size(&self) -> usize {
        match *self {
            ObjectType::GigaPageObject => SEL4_HUGE_PAGE_BITS,
            ObjectType::NormalPageObject => SEL4_PAGE_BITS,
            ObjectType::MegaPageObject => SEL4_LARGE_PAGE_BITS,
            ObjectType::PageTableObject => SEL4_PAGE_BITS,
            _ => panic!("unsupported cap type:{}", (*self) as usize),
        }
    }

    /// Returns the frame type of the object.
    ///
    /// # Returns
    ///
    /// The frame type of the object.
    pub fn get_frame_type(&self) -> usize {
        match self {
            ObjectType::NormalPageObject => RISCV_4K_PAGE,
            ObjectType::MegaPageObject => RISCV_MEGA_PAGE,
            ObjectType::GigaPageObject => RISCV_GIGA_PAGE,
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
            Self::GigaPageObject | Self::NormalPageObject | Self::MegaPageObject
        )
    }
}
