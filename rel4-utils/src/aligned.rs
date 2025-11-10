//! This module provide a wapper for structure aligned.
//!
//!
use core::ops::{Deref, DerefMut};

/// This is a wrapper to align inner structure with 4K.
#[repr(align(4096))]
pub struct Align4K<T>(T);

impl<T> Align4K<T> {
    pub const fn new(v: T) -> Self {
        Self(v)
    }
}

impl<T> Deref for Align4K<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Align4K<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
