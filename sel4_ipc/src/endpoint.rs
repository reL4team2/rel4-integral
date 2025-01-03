use crate::transfer::Transfer;
use sel4_common::arch::ArchReg;
use sel4_common::structures_gen::endpoint;
#[cfg(feature = "KERNEL_MCS")]
use sel4_common::structures_gen::seL4_Fault_tag::seL4_Fault_NullFault;
use sel4_common::utils::{convert_to_mut_type_ref, convert_to_option_mut_type_ref};
#[cfg(feature = "KERNEL_MCS")]
use sel4_task::{ksCurSC, reply::reply_t, sched_context::sched_context_t};
use sel4_task::{
    possible_switch_to, rescheduleRequired, schedule_tcb, set_thread_state, tcb_queue_t, tcb_t,
    ThreadState,
};
use sel4_vspace::pptr_t;

pub const EPState_Idle: usize = EPState::Idle as usize;
pub const EPState_Send: usize = EPState::Send as usize;
pub const EPState_Recv: usize = EPState::Recv as usize;

#[derive(PartialEq, Eq, Debug)]
/// The state of an endpoint
pub enum EPState {
    Idle = 0,
    Send = 1,
    Recv = 2,
}
#[cfg(feature = "KERNEL_MCS")]
use sel4_common::structures_gen::cap_reply_cap;

pub trait endpoint_func {
    fn get_ptr(&self) -> pptr_t;
    fn get_ep_state(&self) -> EPState;
    fn get_queue(&self) -> tcb_queue_t;
    fn set_queue(&mut self, tcb_queue: &tcb_queue_t);
    fn cancel_ipc(&mut self, tcb: &mut tcb_t);
    fn cancel_all_ipc(&mut self);
    fn cancel_badged_sends(&mut self, badge: usize);
    #[cfg(not(feature = "KERNEL_MCS"))]
    fn send_ipc(
        &mut self,
        src_thread: &mut tcb_t,
        blocking: bool,
        do_call: bool,
        can_grant: bool,
        badge: usize,
        can_grant_reply: bool,
    );
    #[cfg(feature = "KERNEL_MCS")]
    fn send_ipc(
        &mut self,
        src_thread: &mut tcb_t,
        blocking: bool,
        do_call: bool,
        can_grant: bool,
        badge: usize,
        can_grant_reply: bool,
        canDonate: bool,
    );
    #[cfg(not(feature = "KERNEL_MCS"))]
    fn receive_ipc(&mut self, thread: &mut tcb_t, is_blocking: bool, grant: bool);
    #[cfg(feature = "KERNEL_MCS")]
    fn receive_ipc(&mut self, thread: &mut tcb_t, is_blocking: bool, Option_reply_cap: Option<&mut cap_reply_cap>);
    #[cfg(feature = "KERNEL_MCS")]
    fn reorder_EP(&mut self, thread: &mut tcb_t);
}
impl endpoint_func for endpoint {
    #[inline]
    /// Get the raw pointer(usize) to the endpoint
    fn get_ptr(&self) -> pptr_t {
        self as *const Self as pptr_t
    }

    #[inline]
    /// Get the state of the endpoint
    fn get_ep_state(&self) -> EPState {
        unsafe { core::mem::transmute::<u8, EPState>(self.get_state() as u8) }
    }

    #[inline]
    /// Get the tcb queue of the queue
    fn get_queue(&self) -> tcb_queue_t {
        tcb_queue_t {
            head: self.get_epQueue_head() as usize,
            tail: self.get_epQueue_tail() as usize,
        }
    }

    #[inline]
    /// Set the tcb queue to the queue
    fn set_queue(&mut self, tcb_queue: &tcb_queue_t) {
        self.set_epQueue_head(tcb_queue.head as u64);
        self.set_epQueue_tail(tcb_queue.tail as u64);
    }

    #[inline]
    /// Cancel the IPC of the tcb in the endpoint, and set the tcb to inactive
    /// # Arguments
    /// * `tcb` - The tcb to cancel the IPC
    fn cancel_ipc(&mut self, tcb: &mut tcb_t) {
        let mut queue = self.get_queue();
        queue.ep_dequeue(tcb);
        self.set_queue(&queue);
        if queue.head == 0 {
            self.set_state(EPState::Idle as u64);
        }
        #[cfg(feature = "KERNEL_MCS")]
        {
            if let Some(reply) =
                convert_to_option_mut_type_ref::<reply_t>(tcb.tcbState.get_replyObject() as usize)
            {
                reply.unlink(tcb);
            }
        }
        set_thread_state(tcb, ThreadState::ThreadStateInactive);
    }

    #[inline]
    /// Cancel all IPC in the endpoint
    fn cancel_all_ipc(&mut self) {
        match self.get_ep_state() {
            EPState::Idle => {}
            _ => {
                let mut op_thread =
                    convert_to_option_mut_type_ref::<tcb_t>(self.get_epQueue_head() as usize);
                self.set_state(EPState::Idle as u64);
                self.set_epQueue_head(0);
                self.set_epQueue_tail(0);
                while let Some(thread) = op_thread {
                    #[cfg(feature = "KERNEL_MCS")]
                    {
                        let reply_ptr = thread.tcbState.get_replyObject() as usize;
                        if reply_ptr != 0 {
                            convert_to_mut_type_ref::<reply_t>(reply_ptr).unlink(thread);
                        }
                        if thread.tcbFault.get_tag() == seL4_Fault_NullFault as u64 {
                            set_thread_state(thread, ThreadState::ThreadStateRestart);
                            if thread.tcbSchedContext != 0
                                && convert_to_mut_type_ref::<sched_context_t>(
                                    thread.tcbSchedContext,
                                )
                                .sc_sporadic()
                            {
                                assert!(thread.tcbSchedContext != unsafe { ksCurSC });
                                if thread.tcbSchedContext != unsafe { ksCurSC } {
                                    convert_to_mut_type_ref::<sched_context_t>(
                                        thread.tcbSchedContext,
                                    )
                                    .refill_unblock_check();
                                }
                            }
                            possible_switch_to(thread);
                        } else {
                            set_thread_state(thread, ThreadState::ThreadStateInactive);
                        }
                    }
                    #[cfg(not(feature = "KERNEL_MCS"))]
                    {
                        set_thread_state(thread, ThreadState::ThreadStateRestart);
                        thread.sched_enqueue();
                    }

                    op_thread = convert_to_option_mut_type_ref::<tcb_t>(thread.tcbEPNext);
                }
                rescheduleRequired();
            }
        }
    }

    /// Cancel badged sends in the endpoint, and set the tcb to restart
    /// # Arguments
    /// * `badge` - The badge to cancel
    fn cancel_badged_sends(&mut self, badge: usize) {
        match self.get_ep_state() {
            EPState::Idle | EPState::Recv => {}
            EPState::Send => {
                let mut queue = self.get_queue();
                self.set_state(EPState::Idle as u64);
                self.set_epQueue_head(0);
                self.set_epQueue_tail(0);
                let mut thread_ptr = queue.head;
                while thread_ptr != 0 {
                    let thread = convert_to_mut_type_ref::<tcb_t>(thread_ptr);
                    thread_ptr = thread.tcbEPNext;
                    #[cfg(feature = "KERNEL_MCS")]
                    {
                        assert!(thread.tcbState.get_replyObject() == 0);
                    }
                    if thread.tcbState.get_blockingIPCBadge() as usize == badge {
                        #[cfg(not(feature = "KERNEL_MCS"))]
                        {
                            set_thread_state(thread, ThreadState::ThreadStateRestart);
                            thread.sched_enqueue();
                            queue.ep_dequeue(thread);
                        }
                        #[cfg(feature = "KERNEL_MCS")]
                        {
                            if thread.tcbFault.get_tag() == seL4_Fault_NullFault {
                                set_thread_state(thread, ThreadState::ThreadStateRestart);
                                if convert_to_mut_type_ref::<sched_context_t>(
                                    thread.tcbSchedContext,
                                )
                                .sc_sporadic()
                                {
                                    assert!(thread.tcbSchedContext != unsafe { ksCurSC });
                                    if thread.tcbSchedContext != unsafe { ksCurSC } {
                                        convert_to_mut_type_ref::<sched_context_t>(
                                            thread.tcbSchedContext,
                                        )
                                        .refill_unblock_check();
                                    }
                                }
                                possible_switch_to(thread);
                            } else {
                                set_thread_state(thread, ThreadState::ThreadStateInactive);
                            }
                            queue.ep_dequeue(thread);
                        }
                    }
                }
                self.set_queue(&queue);
                if queue.head != 0 {
                    self.set_state(EPState::Send as u64);
                }
                rescheduleRequired();
            }
        }
    }

    /// Send an IPC to the endpoint, if the endpoint is idle or send, the tcb will be blocked immediately
    /// , otherwise the thread will do ipc transfer to the destination thread(queue head)
    /// * `src_thread` - The source thread to send the IPC
    /// * `blocking` - If the IPC is blocking
    /// * `do_call` - If the IPC is a call
    /// * `can_grant` - If the IPC can grant
    /// * `badge` - The badge of the IPC
    /// * `can_grant_reply` - If the IPC can grant the reply
    #[cfg(not(feature = "KERNEL_MCS"))]
    fn send_ipc(
        &mut self,
        src_thread: &mut tcb_t,
        blocking: bool,
        do_call: bool,
        can_grant: bool,
        badge: usize,
        can_grant_reply: bool,
    ) {
        match self.get_ep_state() {
            EPState::Idle | EPState::Send => {
                if blocking {
                    src_thread
                        .tcbState
                        .set_tsType(ThreadState::ThreadStateBlockedOnSend as u64);
                    src_thread
                        .tcbState
                        .set_blockingObject(self.get_ptr() as u64);
                    src_thread
                        .tcbState
                        .set_blockingIPCCanGrant(can_grant as u64);
                    src_thread.tcbState.set_blockingIPCBadge(badge as u64);
                    src_thread
                        .tcbState
                        .set_blockingIPCCanGrantReply(can_grant_reply as u64);
                    src_thread.tcbState.set_blockingIPCIsCall(do_call as u64);
                    schedule_tcb(src_thread);

                    let mut queue = self.get_queue();
                    queue.ep_append(src_thread);
                    self.set_state(EPState::Send as u64);
                    self.set_queue(&queue);
                }
            }

            EPState::Recv => {
                let mut queue = self.get_queue();
                let op_dest_thread = convert_to_option_mut_type_ref::<tcb_t>(queue.head);
                assert!(op_dest_thread.is_some());
                let dest_thread = op_dest_thread.unwrap();
                queue.ep_dequeue(dest_thread);
                self.set_queue(&queue);
                if queue.empty() {
                    self.set_state(EPState::Idle as u64);
                }
                src_thread.do_ipc_transfer(dest_thread, Some(self), badge, can_grant);
                let reply_can_grant = dest_thread.tcbState.get_blockingIPCCanGrant() != 0;
                set_thread_state(dest_thread, ThreadState::ThreadStateRunning);
                possible_switch_to(dest_thread);
                if do_call {
                    if can_grant || can_grant_reply {
                        dest_thread.setup_caller_cap(src_thread, reply_can_grant);
                    } else {
                        set_thread_state(src_thread, ThreadState::ThreadStateInactive);
                    }
                }
            }
        }
    }
    // TODO: MCS
    #[cfg(feature = "KERNEL_MCS")]
    fn send_ipc(
        &mut self,
        src_thread: &mut tcb_t,
        blocking: bool,
        do_call: bool,
        can_grant: bool,
        badge: usize,
        can_grant_reply: bool,
        canDonate: bool,
    ) {
        match self.get_ep_state() {
            EPState::Idle | EPState::Send => {
                if blocking {
                    src_thread
                        .tcbState
                        .set_tsType(ThreadState::ThreadStateBlockedOnSend as u64);
                    src_thread
                        .tcbState
                        .set_blockingObject(self.get_ptr() as u64);
                    src_thread
                        .tcbState
                        .set_blockingIPCCanGrant(can_grant as u64);
                    src_thread.tcbState.set_blockingIPCBadge(badge as u64);
                    src_thread
                        .tcbState
                        .set_blockingIPCCanGrantReply(can_grant_reply as u64);
                    src_thread.tcbState.set_blockingIPCIsCall(do_call as u64);
                    schedule_tcb(src_thread);

                    let mut queue = self.get_queue();
                    queue.ep_append(src_thread);
                    self.set_state(EPState::Send as u64);
                    self.set_queue(&queue);
                }
            }

            EPState::Recv => {
                let mut queue = self.get_queue();
                let op_dest_thread = convert_to_option_mut_type_ref::<tcb_t>(queue.head);
                assert!(op_dest_thread.is_some());
                let dest_thread = op_dest_thread.unwrap();
                queue.ep_dequeue(dest_thread);
                self.set_queue(&queue);
                if queue.empty() {
                    self.set_state(EPState::Idle as u64);
                }
                src_thread.do_ipc_transfer(dest_thread, Some(self), badge, can_grant);

                let replyptr = dest_thread.tcbState.get_replyObject() as usize;
                if replyptr != 0 {
                    convert_to_mut_type_ref::<reply_t>(replyptr).unlink(dest_thread);
                }
                if do_call || src_thread.tcbFault.get_tag() != seL4_Fault_NullFault {
                    if replyptr != 0 && (can_grant || can_grant_reply) {
                        convert_to_mut_type_ref::<reply_t>(replyptr).push(
                            src_thread,
                            dest_thread,
                            canDonate,
                        );
                    } else {
                        set_thread_state(dest_thread, ThreadState::ThreadStateInactive);
                    }
                } else if canDonate && dest_thread.tcbSchedContext == 0 {
                    convert_to_mut_type_ref::<sched_context_t>(src_thread.tcbSchedContext)
                        .schedContext_donate(dest_thread);
                }

                assert!(
                    dest_thread.tcbSchedContext == 0
                        || convert_to_mut_type_ref::<sched_context_t>(dest_thread.tcbSchedContext)
                            .refill_sufficient(0)
                );
                assert!(
                    dest_thread.tcbSchedContext == 0
                        || convert_to_mut_type_ref::<sched_context_t>(dest_thread.tcbSchedContext)
                            .refill_ready()
                );
                set_thread_state(dest_thread, ThreadState::ThreadStateRunning);
                if let Some(sc) =
                    convert_to_option_mut_type_ref::<sched_context_t>(dest_thread.tcbSchedContext)
                {
                    if sc.sc_sporadic() && dest_thread.tcbSchedContext != unsafe { ksCurSC } {
                        sc.refill_unblock_check();
                    }
                }
                possible_switch_to(dest_thread);
            }
        }
    }

    /// Receive an IPC from the endpoint, if the endpoint is idle or recv, the tcb will be blocked immediately
    /// , otherwise the thread will be transferred from the src thread(queue head)
    /// # Arguments
    /// * `thread` - The thread to receive the IPC
    /// * `is_blocking` - If the IPC is blocking
    /// * `grant` - If the IPC can grant
    #[cfg(not(feature = "KERNEL_MCS"))]
    fn receive_ipc(&mut self, thread: &mut tcb_t, is_blocking: bool, grant: bool) {
        if thread.complete_signal() {
            return;
        }
        match self.get_ep_state() {
            EPState::Idle | EPState::Recv => {
                if is_blocking {
                    thread.tcbState.set_blockingObject(self.get_ptr() as u64);
                    thread.tcbState.set_blockingIPCCanGrant(grant as u64);
                    set_thread_state(thread, ThreadState::ThreadStateBlockedOnReceive);
                    let mut queue = self.get_queue();
                    queue.ep_append(thread);
                    self.set_state(EPState::Recv as u64);
                    self.set_queue(&queue);
                } else {
                    // NBReceive failed
                    thread.tcbArch.set_register(ArchReg::Badge, 0);
                }
            }
            EPState::Send => {
                let mut queue = self.get_queue();
                assert!(!queue.empty());
                let sender = convert_to_mut_type_ref::<tcb_t>(queue.head);
                queue.ep_dequeue(sender);
                self.set_queue(&queue);
                if queue.empty() {
                    self.set_state(EPState::Idle as u64);
                }
                let badge = sender.tcbState.get_blockingIPCBadge() as usize;
                let can_grant = sender.tcbState.get_blockingIPCCanGrant() != 0;
                let can_grant_reply = sender.tcbState.get_blockingIPCCanGrantReply() != 0;
                sender.do_ipc_transfer(thread, Some(self), badge, can_grant);
                let do_call = sender.tcbState.get_blockingIPCIsCall() != 0;
                if do_call {
                    if can_grant || can_grant_reply {
                        thread.setup_caller_cap(sender, grant);
                    } else {
                        set_thread_state(sender, ThreadState::ThreadStateInactive);
                    }
                } else {
                    set_thread_state(sender, ThreadState::ThreadStateRunning);
                    possible_switch_to(sender);
                }
            }
        }
    }
    //TODO: MCS
    #[cfg(feature = "KERNEL_MCS")]
    fn receive_ipc(
        &mut self,
        thread: &mut tcb_t,
        is_blocking: bool,
        Option_reply_cap: Option<&mut cap_reply_cap>,
    ) {
        use core::intrinsics::unlikely;
        use log::debug;
        use sel4_common::structures_gen::{notification_t, seL4_Fault_tag};

        use crate::notification_func;
        let mut replyptr: usize = 0;
        if let Some(reply_cap_data) = Option_reply_cap {
            replyptr = reply_cap_data.get_capReplyPtr() as usize;
            let reply = convert_to_mut_type_ref::<reply_t>(replyptr);
            if unlikely(reply.replyTCB != 0 && reply.replyTCB != thread.get_ptr()) {
                debug!("Reply object already has unexecuted reply!");
                convert_to_mut_type_ref::<tcb_t>(reply.replyTCB as usize).cancel_ipc();
            }
        }
        if thread.complete_signal() {
            return;
        }
        if thread.tcbBoundNotification != 0 && is_blocking {
            convert_to_mut_type_ref::<notification_t>(thread.tcbBoundNotification)
                .maybeReturnSchedContext(thread);
        }
        match self.get_ep_state() {
            EPState::Idle | EPState::Recv => {
                if is_blocking {
                    thread
                        .tcbState
                        .set_tsType(ThreadState::ThreadStateBlockedOnReceive as u64);
                    thread.tcbState.set_blockingObject(self.get_ptr() as u64);
                    // MCS
                    thread.tcbState.set_replyObject(replyptr as u64);
                    if replyptr != 0 {
                        convert_to_mut_type_ref::<reply_t>(replyptr).replyTCB = thread.get_ptr();
                    }
                    schedule_tcb(&thread);
                    let mut queue = self.get_queue();
                    queue.ep_append(thread);
                    self.set_state(EPState::Recv as u64);
                    self.set_queue(&queue);
                } else {
                    // NBReceive failed
                    thread.tcbArch.set_register(ArchReg::Badge, 0);
                }
            }
            EPState::Send => {
                let mut queue = self.get_queue();
                assert!(!queue.empty());
                let sender = convert_to_mut_type_ref::<tcb_t>(queue.head);
                queue.ep_dequeue(sender);
                self.set_queue(&queue);
                if queue.empty() {
                    self.set_state(EPState::Idle as u64);
                }
                let badge = sender.tcbState.get_blockingIPCBadge() as usize;
                let can_grant = sender.tcbState.get_blockingIPCCanGrant() != 0;
                let can_grant_reply = sender.tcbState.get_blockingIPCCanGrantReply() != 0;
                sender.do_ipc_transfer(thread, Some(self), badge, can_grant);
                let do_call = sender.tcbState.get_blockingIPCIsCall() != 0;
                // MCS
                if let Some(sc) =
                    convert_to_option_mut_type_ref::<sched_context_t>(sender.tcbSchedContext)
                {
                    if sc.sc_sporadic() {
                        unsafe {
                            assert!(sender.tcbSchedContext != ksCurSC);
                            if sender.tcbSchedContext != ksCurSC {
                                sc.refill_unblock_check();
                            }
                        }
                    }
                }
                if do_call || sender.tcbFault.get_tag() != seL4_Fault_tag::seL4_Fault_NullFault {
                    if (can_grant || can_grant_reply) && replyptr != 0 {
                        let canDonate = sender.tcbSchedContext != 0
                            && sender.tcbFault.get_tag() != seL4_Fault_tag::seL4_Fault_Timeout;
                        convert_to_mut_type_ref::<reply_t>(replyptr)
                            .push(sender, thread, canDonate);
                    } else {
                        set_thread_state(sender, ThreadState::ThreadStateInactive);
                    }
                } else {
                    set_thread_state(sender, ThreadState::ThreadStateRunning);
                    possible_switch_to(sender);
                    assert!(
                        sender.tcbSchedContext == 0
                            || convert_to_mut_type_ref::<sched_context_t>(sender.tcbSchedContext)
                                .refill_sufficient(0)
                    );
                }
            }
        }
    }
    #[cfg(feature = "KERNEL_MCS")]
    #[no_mangle]
    fn reorder_EP(&mut self, thread: &mut tcb_t) {
        let mut queue = self.get_queue();
        queue.ep_dequeue(thread);
        queue.ep_append(thread);
        self.set_queue(&queue);
    }
}
