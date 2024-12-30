use sel4_common::utils::convert_to_mut_type_ref;

use super::tcb::tcb_t;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Structure for the tcb queue
pub struct tcb_queue_t {
    /// The head of the queue
    pub head: usize,
    /// The tail of the queue
    pub tail: usize,
}

impl tcb_queue_t {
    /// Append a tcb to the queue
    #[cfg(not(feature = "KERNEL_MCS"))]
    pub fn ep_append(&mut self, tcb: &mut tcb_t) {
        if self.head == 0 {
            self.head = tcb.get_ptr();
        } else {
            convert_to_mut_type_ref::<tcb_t>(self.tail).tcbEPNext = tcb.get_ptr();
        }

        tcb.tcbEPPrev = self.tail;
        tcb.tcbEPNext = 0;
        self.tail = tcb.get_ptr();
    }
    #[cfg(feature = "KERNEL_MCS")]
    pub fn ep_append(&mut self, tcb: &mut tcb_t) {
        use core::intrinsics::{likely, unlikely};

        let mut before_ptr: usize = self.tail;
        let mut after_ptr: usize = 0;

        while unlikely(
            before_ptr != 0
                && tcb.tcbPriority > convert_to_mut_type_ref::<tcb_t>(before_ptr).tcbPriority,
        ) {
            after_ptr = before_ptr;
            before_ptr = convert_to_mut_type_ref::<tcb_t>(after_ptr).tcbEPPrev;
        }
        if unlikely(before_ptr == 0) {
            self.head = tcb.get_ptr();
        } else {
            convert_to_mut_type_ref::<tcb_t>(before_ptr).tcbEPNext = tcb.get_ptr()
        }

        if likely(after_ptr == 0) {
            self.tail = tcb.get_ptr();
        } else {
            convert_to_mut_type_ref::<tcb_t>(after_ptr).tcbEPPrev = tcb.get_ptr();
        }

        tcb.tcbEPNext = after_ptr;
        tcb.tcbEPPrev = before_ptr;
    }

    /// Dequeue a tcb from the queue
    pub fn ep_dequeue(&mut self, tcb: &mut tcb_t) {
        if tcb.tcbEPPrev != 0 {
            convert_to_mut_type_ref::<tcb_t>(tcb.tcbEPPrev).tcbEPNext = tcb.tcbEPNext;
        } else {
            self.head = tcb.tcbEPNext;
        }

        if tcb.tcbEPNext != 0 {
            convert_to_mut_type_ref::<tcb_t>(tcb.tcbEPNext).tcbEPPrev = tcb.tcbEPPrev;
        } else {
            self.tail = tcb.tcbEPPrev;
        }
    }

    #[inline]
    /// Check if the queue is empty
    pub fn empty(&self) -> bool {
        return self.head == 0;
    }
}
