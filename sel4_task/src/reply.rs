use core::intrinsics::likely;

use sel4_common::{structures_gen::call_stack, utils::convert_to_mut_type_ref};

use crate::{sched_context::sched_context_t, set_thread_state, tcb_t, ThreadState};

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
impl reply {
    pub fn get_ptr(&mut self) -> usize {
        self as *const _ as usize
    }
    pub fn unlink(&mut self, tcb: &mut tcb_t) {
        assert!(self.replyTCB == tcb.get_ptr());
        assert!(tcb.tcbState.get_replyObject() as usize == self.get_ptr());
        tcb.tcbState.set_replyObject(0);
        self.replyTCB = 0;
        set_thread_state(tcb, ThreadState::ThreadStateInactive);
    }
    pub fn push(&mut self, tcb_caller: &mut tcb_t, tcb_callee: &mut tcb_t, canDonate: bool) {
        assert!(tcb_caller.get_ptr() != 0);
        assert!(self.get_ptr() != 0);
        assert!(self.replyTCB == 0);

        assert!(self.replyPrev.get_callStackPtr() == 0);
        assert!(self.replyNext.get_callStackPtr() == 0);

        /* tcb caller should not be in a existing call stack */
        assert!(tcb_caller.tcbState.get_replyObject() == 0);

        /* unlink callee and reply - they may not have been linked already,
        	* if this rendesvous is occuring when seL4_Recv is called,
        	* however, no harm in overring 0 with 0 */
        tcb_callee.tcbState.set_replyObject(0);

        /* link caller and reply */
        self.replyTCB = tcb_caller.get_ptr();
        tcb_caller.tcbState.set_replyObject(self.get_ptr() as u64);
        set_thread_state(tcb_caller, ThreadState::ThreadStateBlockedOnReply);

        if tcb_caller.tcbSchedContext != 0 && tcb_callee.tcbSchedContext == 0 && canDonate {
            let sc_donated = convert_to_mut_type_ref::<sched_context_t>(tcb_caller.tcbSchedContext);

            /* check stack integrity */
            assert!(
                sc_donated.scReply == 0
                    || convert_to_mut_type_ref::<reply_t>(sc_donated.scReply)
                        .replyNext
                        .get_callStackPtr()
                        == tcb_caller.tcbSchedContext as u64
            );

            /* push on to stack */
            self.replyPrev = call_stack::new(0, sc_donated.scReply as u64);
            if sc_donated.scReply != 0 {
                convert_to_mut_type_ref::<reply_t>(sc_donated.scReply).replyNext =
                    call_stack::new(0, self.get_ptr() as u64);
            }
            self.replyNext = call_stack::new(1, sc_donated.get_ptr() as u64);
            sc_donated.scReply = self.get_ptr();

            /* now do the actual donation */
            sc_donated.schedContext_donate(tcb_callee);
        }
    }
    pub fn pop(&mut self, tcb: &mut tcb_t) {
        assert!(self.get_ptr() != 0);
        assert!(self.replyTCB == tcb.get_ptr());
        assert!(tcb.tcbState.get_tsType() == ThreadState::ThreadStateBlockedOnReply as u64);
        assert!(tcb.tcbState.get_replyObject() as usize == self.get_ptr());

        let next_ptr = self.replyNext.get_callStackPtr() as usize;
        let prev_ptr = self.replyPrev.get_callStackPtr() as usize;

        if likely(next_ptr != 0) {
            assert!(self.replyNext.get_isHead() != 0);

            convert_to_mut_type_ref::<sched_context_t>(next_ptr).scReply = prev_ptr;
            if prev_ptr != 0 {
                convert_to_mut_type_ref::<reply_t>(prev_ptr).replyNext = self.replyNext.clone();
                assert!(
                    convert_to_mut_type_ref::<reply_t>(prev_ptr)
                        .replyNext
                        .get_isHead()
                        != 0
                );
            }

            /* give it back */
            if tcb.tcbSchedContext == 0 {
                /* only give the SC back if our SC is NULL. This prevents
                	* strange behaviour when a thread is bound to an sc while it is
                	* in the BlockedOnReply state. The semantics in this case are that the
                	* SC cannot go back to the caller if the caller has received another one */
                convert_to_mut_type_ref::<sched_context_t>(next_ptr).schedContext_donate(tcb);
            }
        }

        self.replyPrev = call_stack::new(0, 0);
        self.replyNext = call_stack::new(0, 0);
        self.unlink(tcb);
    }
    pub fn remove(&mut self, tcb: &mut tcb_t) {
        assert!(self.replyTCB == tcb.get_ptr());
        assert!(tcb.tcbState.get_tsType() == ThreadState::ThreadStateBlockedOnReply as u64);
        assert!(tcb.tcbState.get_replyObject() == self.get_ptr() as u64);

        let next_ptr = self.replyNext.get_callStackPtr() as usize;
        let prev_ptr = self.replyPrev.get_callStackPtr() as usize;

        if likely(next_ptr != 0 && self.replyNext.get_isHead() != 0) {
            /* head of the call stack -> just pop */
            self.pop(tcb);
        } else {
            if next_ptr != 0 {
                /* not the head, remove from middle - break the chain */
                convert_to_mut_type_ref::<reply_t>(next_ptr).replyPrev = call_stack::new(0, 0);
            }
            if prev_ptr != 0 {
                convert_to_mut_type_ref::<reply_t>(prev_ptr).replyNext = call_stack::new(0, 0);
            }
            self.replyPrev = call_stack::new(0, 0);
            self.replyNext = call_stack::new(0, 0);
            self.unlink(tcb);
        }
    }
}
