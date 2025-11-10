use core::ops::{Deref, DerefMut};

use sel4_common::{sel4_config::PT_INDEX_BITS, utils::pageBitsForSize};

#[no_mangle]
pub fn check_vp_alignment(sz: usize, w: usize) -> bool {
    w & mask_bits!(pageBitsForSize(sz)) == 0
}

pub const PAGE_ALIGNED_LEN: usize = bit!(PT_INDEX_BITS);

#[repr(align(4096))]
#[derive(Clone, Copy)]
pub struct PageAligned<T>([T; PAGE_ALIGNED_LEN]);

impl<T: Copy> PageAligned<T> {
    pub const fn new(v: T) -> Self {
        Self([v; PAGE_ALIGNED_LEN])
    }
}

impl<T> Deref for PageAligned<T> {
    type Target = [T; PAGE_ALIGNED_LEN];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for PageAligned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
