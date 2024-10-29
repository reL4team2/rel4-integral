use crate::transfer::Transfer;
use sel4_common::arch::ArchReg;
use sel4_common::structures_gen::notification;
use sel4_common::utils::{convert_to_mut_type_ref, convert_to_option_mut_type_ref};
use sel4_task::{
    possible_switch_to, rescheduleRequired, set_thread_state, tcb_queue_t, tcb_t, ThreadState,
};

#[derive(PartialEq, Eq, Debug)]
/// The state of a notification
pub enum NtfnState {
    Idle = 0,
    Waiting = 1,
    Active = 2,
}

pub trait notification_func {
    fn get_ntfn_state(&self) -> NtfnState;
    fn get_queue(&self) -> tcb_queue_t;
    fn set_queue(&mut self, queue: &tcb_queue_t);
    fn active(&mut self, badge: usize);
    fn cancel_signal(&mut self, tcb: &mut tcb_t);
    fn cacncel_all_signal(&mut self);
    fn bind_tcb(&mut self, tcb: &mut tcb_t);
    fn unbind_tcb(&mut self);
    fn safe_unbind_tcb(&mut self);
    fn get_ptr(&self) -> usize;
    fn send_signal(&mut self, badge: usize);
    fn receive_signal(&mut self, recv_thread: &mut tcb_t, is_blocking: bool);
}
impl notification_func for notification {
    #[inline]
    /// Get the state of the notification
    fn get_ntfn_state(&self) -> NtfnState {
        unsafe { core::mem::transmute::<u8, NtfnState>(self.get_state() as u8) }
    }

    #[inline]
    /// Get the tcb queue of the notification
    fn get_queue(&self) -> tcb_queue_t {
        tcb_queue_t {
            head: self.get_ntfnQueue_head() as usize,
            tail: self.get_ntfnQueue_tail() as usize,
        }
    }

    #[inline]
    /// Set the tcb queue to the notification
    fn set_queue(&mut self, queue: &tcb_queue_t) {
        self.set_ntfnQueue_head(queue.head as u64);
        self.set_ntfnQueue_tail(queue.tail as u64);
    }

    #[inline]
    /// Set the notification to active
    /// # Arguments
    /// * `badge` - The badge to set
    fn active(&mut self, badge: usize) {
        self.set_state(NtfnState::Active as u64);
        self.set_ntfnMsgIdentifier(badge as u64);
    }

    #[inline]
    /// Cancel the signal of the tcb in the notification
    /// # Arguments
    /// * `tcb` - The tcb to cancel
    fn cancel_signal(&mut self, tcb: &mut tcb_t) {
        let mut queue = self.get_queue();
        queue.ep_dequeue(tcb);
        self.set_queue(&queue);
        if queue.head == 0 {
            self.set_state(NtfnState::Idle as u64);
        }
        set_thread_state(tcb, ThreadState::ThreadStateInactive);
    }

    #[inline]
    /// Cancel all signal in the notification
    fn cacncel_all_signal(&mut self) {
        if self.get_ntfn_state() == NtfnState::Waiting {
            let mut op_thread =
                convert_to_option_mut_type_ref::<tcb_t>(self.get_ntfnQueue_head() as usize);
            self.set_state(NtfnState::Idle as u64);
            self.set_ntfnQueue_head(0);
            self.set_ntfnQueue_tail(0);
            while let Some(thread) = op_thread {
                set_thread_state(thread, ThreadState::ThreadStateRestart);
                thread.sched_enqueue();
                op_thread = convert_to_option_mut_type_ref::<tcb_t>(thread.tcbEPNext);
            }
            rescheduleRequired();
        }
    }

    #[inline]
    /// Bind the tcb to the notification
    fn bind_tcb(&mut self, tcb: &mut tcb_t) {
        self.set_ntfnBoundTCB(tcb.get_ptr() as u64);
    }

    #[inline]
    /// Unbind the tcb to the notification
    fn unbind_tcb(&mut self) {
        self.set_ntfnBoundTCB(0);
    }

    #[inline]
    /// Safely unbind the tcb to the notification
    fn safe_unbind_tcb(&mut self) {
        let tcb = self.get_ntfnBoundTCB() as usize;
        self.unbind_tcb();
        if tcb != 0 {
            convert_to_mut_type_ref::<tcb_t>(tcb).unbind_notification();
        }
    }

    #[inline]
    /// Get the raw pointer of the notification
    fn get_ptr(&self) -> usize {
        self as *const notification as usize
    }

    #[inline]
    /// Send a signal to the notification.
    /// 1: If the notification is idle, the badge is sent to the bound tcb if it exists, otherwise the notification is set to active.
    /// 2: If the notification is waiting, the badge is sent to the head of the queue.
    /// 3: If the notification is active, the badge is added to the message identifier.
    /// # Arguments
    /// * `badge` - The badge to send
    fn send_signal(&mut self, badge: usize) {
        match self.get_ntfn_state() {
            NtfnState::Idle => {
                if let Some(tcb) =
                    convert_to_option_mut_type_ref::<tcb_t>(self.get_ntfnBoundTCB() as usize)
                {
                    if tcb.get_state() == ThreadState::ThreadStateBlockedOnReceive {
                        tcb.cancel_ipc();
                        set_thread_state(tcb, ThreadState::ThreadStateRunning);
                        tcb.tcbArch.set_register(ArchReg::Badge, badge);
                        possible_switch_to(tcb);
                    } else {
                        self.active(badge);
                    }
                } else {
                    self.active(badge);
                }
            }
            NtfnState::Waiting => {
                let mut queue = self.get_queue();
                if let Some(dest) = convert_to_option_mut_type_ref::<tcb_t>(queue.head) {
                    queue.ep_dequeue(dest);
                    self.set_queue(&queue);
                    if queue.empty() {
                        self.set_state(NtfnState::Idle as u64);
                    }
                    set_thread_state(dest, ThreadState::ThreadStateRunning);
                    dest.tcbArch.set_register(ArchReg::Badge, badge);
                    possible_switch_to(dest);
                } else {
                    panic!("queue is empty!")
                }
            }
            NtfnState::Active => {
                let mut badge2 = self.get_ntfnMsgIdentifier() as usize;
                badge2 |= badge;
                self.set_ntfnMsgIdentifier(badge2 as u64);
            }
        }
    }

    /// Receive a signal from the notification.
    /// 1: If the notification is idle or waiting, the receive thread is blocked immediately.
    /// 2: If the notification is active, the badge is sent to the receive thread.
    /// # Arguments
    /// * `recv_thread` - The thread to receive the signal
    /// * `is_blocking` - If the signal is blocking
    fn receive_signal(&mut self, recv_thread: &mut tcb_t, is_blocking: bool) {
        match self.get_ntfn_state() {
            NtfnState::Idle | NtfnState::Waiting => {
                if is_blocking {
                    recv_thread
                        .tcbState
                        .set_blockingObject(self.get_ptr() as u64);
                    set_thread_state(recv_thread, ThreadState::ThreadStateBlockedOnNotification);
                    let mut queue = self.get_queue();
                    queue.ep_append(recv_thread);
                    self.set_state(NtfnState::Waiting as u64);
                    self.set_queue(&queue);
                } else {
                    recv_thread.tcbArch.set_register(ArchReg::Badge, 0);
                }
            }

            NtfnState::Active => {
                recv_thread
                    .tcbArch
                    .set_register(ArchReg::Badge, self.get_ntfnMsgIdentifier() as usize);
                self.set_state(NtfnState::Idle as u64);
            }
        }
    }
}
