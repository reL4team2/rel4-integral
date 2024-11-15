use crate::structures_gen::call_stack;

pub type reply_t = reply;
#[repr(C)]
#[derive(Debug, Clone)]
// TODO: MCS
pub struct reply {
    /// TCB pointed to by this reply object
    pub replyTCB: usize,
    pub replyPrev: call_stack,
    pub replyNext: call_stack,
    pub padding: usize,
}
