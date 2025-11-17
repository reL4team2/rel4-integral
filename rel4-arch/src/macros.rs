#[macro_export]
macro_rules! pptr {
    ($val:expr) => {
        $crate::basic::PPtr::new(($val) as usize)
    };
}

#[macro_export]
macro_rules! vptr {
    ($val:expr) => {
        $crate::basic::VPtr::new(($val) as usize)
    };
}

#[macro_export]
macro_rules! paddr {
    ($val:expr) => {
        $crate::basic::PAddr::new(($val) as usize)
    };
}
