//! [NoLock] 用来在需要全局共享变量的情况，需要保证不会发生数据冲突的情况。
//!
//! seL4 使用大内核锁已经保证了部分的可靠，需要慎重使用这个结构。
use core::{cell::UnsafeCell, ops::Deref};

#[repr(transparent)]
pub struct NoLock<T>(UnsafeCell<T>);

unsafe impl<T> Sync for NoLock<T> {}
unsafe impl<T> Send for NoLock<T> {}

impl<T> Deref for NoLock<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0.get() }
    }
}

impl<T> NoLock<T> {
    pub const fn new(val: T) -> Self {
        Self(UnsafeCell::new(val))
    }

    /// 返回可变引用，需要保证真的不需要锁的存在
    #[allow(clippy::mut_from_ref)]
    pub const fn no_lock(&self) -> &mut T {
        unsafe { &mut *self.0.get() }
    }
}
