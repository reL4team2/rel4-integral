#[derive(PartialEq, PartialOrd, Debug)]
/// The state of a thread
pub enum ThreadState {
    ThreadStateInactive = 0,
    ThreadStateRunning = 1,
    ThreadStateRestart = 2,
    ThreadStateBlockedOnReceive = 3,
    ThreadStateBlockedOnSend = 4,
    ThreadStateBlockedOnReply = 5,
    ThreadStateBlockedOnNotification = 6,
    ThreadStateIdleThreadState = 7,
    ThreadStateExited = 8,
}

use sel4_common::structures_gen::thread_state;

pub trait thread_state_func {
    fn get_state(&self) -> ThreadState;
}
impl thread_state_func for thread_state {
    /// Get the state of the thread
    fn get_state(&self) -> ThreadState {
        unsafe { core::mem::transmute::<u8, ThreadState>(self.get_tsType() as u8) }
    }
}
