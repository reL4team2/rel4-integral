use sel4_common::structures_gen::{endpoint, notification};

use crate::tcb_t;

extern "C" {
    // reorder ep and reorder ntfn is a circular reference problem
    pub fn reorder_EP(ep: &mut endpoint, thread: &mut tcb_t);
    pub fn reorder_NTFN(ntfn: &mut notification, thread: &mut tcb_t);
    pub fn endTimeslice(can_timeout_fault: bool);
    pub fn handleTimeout(tptr: &mut tcb_t);
}
