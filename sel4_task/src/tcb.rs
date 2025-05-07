#[cfg(feature = "KERNEL_MCS")]
use crate::ksCurSC;
use crate::tcb_queue::tcb_queue_t;
use crate::{ksReadyQueues, prio_t};
#[cfg(feature = "KERNEL_MCS")]
use crate::{ksReleaseQueue, sched_context::sched_context_t};
use core::intrinsics::{likely, unlikely};
use sel4_common::arch::{
    msgRegisterNum, n_exceptionMessage, n_syscallMessage, vm_rights_t, ArchReg, ArchTCB,
};
use sel4_common::ffi::current_fault;
use sel4_common::message_info::seL4_MessageInfo_func;
use sel4_common::sel4_config::*;
use sel4_common::shared_types_bf_gen::seL4_MessageInfo;
use sel4_common::structures::{exception_t, seL4_IPCBuffer};
use sel4_common::structures_gen::{
    cap, cap_tag, lookup_fault, lookup_fault_Splayed, seL4_Fault, seL4_Fault_CapFault,
    seL4_Fault_tag, thread_state,
};
#[cfg(not(feature = "KERNEL_MCS"))]
use sel4_common::structures_gen::{cap_reply_cap, mdb_node};
use sel4_common::utils::{convert_to_mut_type_ref, pageBitsForSize};
#[cfg(feature = "ENABLE_SMP")]
use sel4_common::BIT;
use sel4_common::MASK;
use sel4_common::{fault::*, println};
#[cfg(feature = "KERNEL_MCS")]
use sel4_common::{platform::time_def::ticks_t, utils::convert_to_option_mut_type_ref};
#[cfg(not(feature = "KERNEL_MCS"))]
use sel4_cspace::interface::cte_insert;
use sel4_cspace::interface::{cte_t, resolve_address_bits};
use sel4_vspace::{pptr_t, set_vm_root};

use super::scheduler::{
    add_to_bitmap, get_currenct_thread, possible_switch_to, ready_queues_index, remove_from_bigmap,
    reschedule_required, schedule_tcb, set_current_thread,
};
use super::structures::lookupSlot_raw_ret_t;

use super::thread_state::*;

#[repr(C)]
#[derive(Debug, Clone)]
/// Structure for the TCB
pub struct tcb_t {
    /// The architecture registers of the TCB
    pub tcbArch: ArchTCB,
    /// The state of the TCB
    pub tcbState: thread_state,
    /// The bound notification of the TCB
    pub tcbBoundNotification: usize,
    /// The fault of the TCB
    pub tcbFault: seL4_Fault,
    /// The lookup fault of the TCB
    pub tcbLookupFailure: lookup_fault,
    /// The domain of the TCB
    pub domain: usize,
    /// The maximum controlled priority of the TCB
    pub tcbMCP: usize,
    /// The priority of the TCB
    pub tcbPriority: usize,
    #[cfg(feature = "KERNEL_MCS")]
    /// scheduling context that this TCB is running on
    pub tcbSchedContext: usize,
    #[cfg(feature = "KERNEL_MCS")]
    /// scheduling context that this TCB yielded to
    pub tcbYieldTo: usize,
    /// The time slice of the TCB
    pub tcbTimeSlice: usize,
    /// The falut handler of the TCB
    pub tcbFaultHandler: usize,
    /// The IPC buffer of the TCB
    pub tcbIPCBuffer: usize,
    /// the affinity of the TCB in SMP
    #[cfg(feature = "ENABLE_SMP")]
    pub tcbAffinity: usize,
    /// The next TCB in the scheduling queue
    pub tcbSchedNext: usize,
    /// The previous TCB in the scheduling queue
    pub tcbSchedPrev: usize,
    /// The next TCB in the EP queue
    pub tcbEPNext: usize,
    /// The previous TCB in the EP queue
    pub tcbEPPrev: usize,
}

impl tcb_t {
    #[inline]
    /// Get i th cspace of the TCB, unmutable reference
    pub fn get_cspace(&mut self, i: usize) -> &'static cte_t {
        unsafe {
            let p = ((self.get_mut_ptr()) & !MASK!(seL4_TCBBits)) as *mut cte_t;
            &*(p.add(i))
        }
    }

    #[inline]
    /// Initialize the TCB
    pub fn init(&mut self) {
        self.tcbArch = ArchTCB::default();
    }

    #[inline]
    /// Get i th cspace of the TCB, mutable reference
    pub fn get_cspace_mut_ref(&mut self, i: usize) -> &'static mut cte_t {
        unsafe {
            let p = ((self as *mut tcb_t as usize) & !MASK!(seL4_TCBBits)) as *mut cte_t;
            &mut *(p.add(i))
        }
    }

    #[inline]
    /// Get the current state of the TCB
    pub fn get_state(&self) -> ThreadState {
        unsafe { core::mem::transmute::<u8, ThreadState>(self.tcbState.get_tsType() as u8) }
    }

    #[inline]
    /// Check if the TCB is stopped by checking the state
    pub fn is_stopped(&self) -> bool {
        match self.get_state() {
            ThreadState::ThreadStateInactive
            | ThreadState::ThreadStateBlockedOnNotification
            | ThreadState::ThreadStateBlockedOnReceive
            | ThreadState::ThreadStateBlockedOnReply
            | ThreadState::ThreadStateBlockedOnSend => true,

            _ => false,
        }
    }

    #[inline]
    /// Check if the TCB is runnable by checking the state
    pub fn is_runnable(&self) -> bool {
        match self.get_state() {
            ThreadState::ThreadStateRunning | ThreadState::ThreadStateRestart => true,
            _ => false,
        }
    }
    #[inline]
    pub fn is_blocked(&self) -> bool {
        match self.get_state() {
            ThreadState::ThreadStateBlockedOnReceive
            | ThreadState::ThreadStateBlockedOnSend
            | ThreadState::ThreadStateBlockedOnNotification
            | ThreadState::ThreadStateBlockedOnReply => true,
            _ => false,
        }
    }
    #[inline]
    #[cfg(not(feature = "KERNEL_MCS"))]
    pub fn is_schedulable(&self) -> bool {
        self.is_runnable()
    }
    #[inline]
    #[cfg(feature = "KERNEL_MCS")]
    pub fn is_schedulable(&self) -> bool {
        self.is_runnable()
            && self.tcbSchedContext != 0
            && convert_to_mut_type_ref::<sched_context_t>(self.tcbSchedContext).scRefillMax > 0
            && self.tcbState.get_tcbInReleaseQueue() == 0
    }

    #[inline]
    /// Check if the TCB is current by comparing the tcb pointer
    pub fn is_current(&self) -> bool {
        self.get_ptr() == get_currenct_thread().get_ptr()
    }

    #[inline]
    pub fn set_mc_priority(&mut self, mcp: prio_t) {
        self.tcbMCP = mcp;
    }

    #[inline]
    #[cfg(not(feature = "KERNEL_MCS"))]
    /// Set the priority of the TCB, and reschedule if the thread is runnable and not current
    pub fn set_priority(&mut self, priority: prio_t) {
        self.sched_dequeue();
        self.tcbPriority = priority;
        if self.is_runnable() {
            if self.is_current() {
                reschedule_required();
            } else {
                possible_switch_to(self)
            }
        }
    }
    #[inline]
    #[cfg(feature = "KERNEL_MCS")]
    pub fn set_priority(&mut self, priority: prio_t) {
        use sel4_common::structures_gen::{endpoint_t, notification_t};

        use crate::{reorder_ep, reorder_ntfn};

        match self.get_state() {
            ThreadState::ThreadStateRunning | ThreadState::ThreadStateRestart => {
                if self.tcbState.get_tcbQueued() != 0 || self.is_current() {
                    self.sched_dequeue();
                    self.tcbPriority = priority;
                    self.sched_enqueue();
                    reschedule_required();
                } else {
                    self.tcbPriority = priority;
                }
            }
            ThreadState::ThreadStateBlockedOnReceive | ThreadState::ThreadStateBlockedOnSend => {
                self.tcbPriority = priority;
                unsafe {
                    reorder_ep(
                        convert_to_mut_type_ref::<endpoint_t>(
                            self.tcbState.get_blockingObject() as usize
                        ),
                        self,
                    )
                };
            }
            ThreadState::ThreadStateBlockedOnNotification => {
                self.tcbPriority = priority;
                unsafe {
                    reorder_ntfn(
                        convert_to_mut_type_ref::<notification_t>(
                            self.tcbState.get_blockingObject() as usize,
                        ),
                        self,
                    )
                };
            }
            _ => {
                self.tcbPriority = priority;
            }
        }
    }

    #[inline]
    /// Bind the notification of the TCB
    /// # Arguments
    /// * `addr` - The address of the notification to bind.
    pub fn bind_notification(&mut self, addr: pptr_t) {
        self.tcbBoundNotification = addr;
    }

    #[inline]
    /// Unbind the notification of the TCB(just set the bound notification to 0)
    pub fn unbind_notification(&mut self) {
        self.tcbBoundNotification = 0;
    }

    #[inline]
    /// Set the domain of the TCB.
    pub fn set_domain(&mut self, dom: usize) {
        self.sched_dequeue();
        self.domain = dom;
        if self.is_schedulable() {
            self.sched_enqueue();
        }

        if self.is_current() {
            reschedule_required();
        }
    }

    /// Enqueue the TCB to the scheduling queue
    pub fn sched_enqueue(&mut self) {
        // let thread = self as *mut tcb_t as usize;
        // sel4_common::println!("{}: sched_enqueue: {:#x}, tcb queued: {}", self.get_cpu(), thread, self.tcbState.get_tcbQueued());
        #[cfg(feature = "KERNEL_MCS")]
        {
            assert!(self.is_schedulable());
            assert!(
                convert_to_mut_type_ref::<sched_context_t>(self.tcbSchedContext)
                    .refill_sufficient(0)
            );
        }

        if self.tcbState.get_tcbQueued() == 0 {
            let dom = self.domain;
            let prio = self.tcbPriority;
            let idx = ready_queues_index(dom, prio);
            let queue = self.get_sched_queue(idx);

            if queue.empty() {
                add_to_bitmap(self.get_cpu(), dom, prio);
            }

            queue.prepend(self);
            // if queue.tail == 0 {
            //     queue.tail = self_ptr as usize;
            //     add_to_bitmap(self.get_cpu(), dom, prio);
            // } else {
            //     convert_to_mut_type_ref::<tcb_t>(queue.head).tcbSchedPrev = self_ptr as usize;
            // }
            // self.tcbSchedPrev = 0;
            // self.tcbSchedNext = queue.head;
            // queue.head = self_ptr as usize;

            unsafe {
                ksReadyQueues[idx] = *queue;
            }
            self.tcbState.set_tcbQueued(1);
        }

        #[cfg(feature = "ENABLE_SMP")]
        self.update_queue();
    }

    #[inline]
    /// Get the scheduling queue by index from ksReadyQueues
    pub fn get_sched_queue(&mut self, index: usize) -> &'static mut tcb_queue_t {
        unsafe {
            #[cfg(feature = "ENABLE_SMP")]
            {
                use super::scheduler::ksSMP;
                &mut ksSMP[self.tcbAffinity].ksReadyQueues[index]
            }
            #[cfg(not(feature = "ENABLE_SMP"))]
            {
                &mut ksReadyQueues[index]
            }
        }
    }

    #[inline]
    /// Get the CPU of the TCB, 0 if not in SMP, tcbAffinity if in SMP
    pub fn get_cpu(&self) -> usize {
        #[cfg(feature = "ENABLE_SMP")]
        {
            self.tcbAffinity
        }
        #[cfg(not(feature = "ENABLE_SMP"))]
        {
            0
        }
    }

    /// Dequeue the TCB from the scheduling queue
    pub fn sched_dequeue(&mut self) {
        // let thread = self as *mut tcb_t as usize;
        // sel4_common::println!("{}: sched_dequeue: {:#x}, tcb queued: {}", self.get_cpu(), thread, self.tcbState.get_tcbQueued());
        if self.tcbState.get_tcbQueued() != 0 {
            let dom = self.domain;
            let prio = self.tcbPriority;
            let idx = ready_queues_index(dom, prio);
            let queue = self.get_sched_queue(idx);

            queue.remove(self);

            unsafe {
                ksReadyQueues[idx] = *queue;
            }

            self.tcbState.set_tcbQueued(0);

            if likely(queue.head == 0) {
                remove_from_bigmap(self.get_cpu(), dom, prio);
            }
        }
    }

    /// Append the TCB to the scheduling queue tail
    /// # Note
    /// This function is as same as `sched_enqueue`, but it is used for the EP queue
    pub fn sched_append(&mut self) {
        // let thread = self as *mut tcb_t as usize;
        // sel4_common::println!("{}: sched_append: {:#x}, tcb queued: {}", self.get_cpu(), thread, self.tcbState.get_tcbQueued());
        #[cfg(feature = "KERNEL_MCS")]
        {
            assert!(self.is_schedulable());
            assert!(
                convert_to_mut_type_ref::<sched_context_t>(self.tcbSchedContext)
                    .refill_sufficient(0)
            );
            assert!(
                convert_to_mut_type_ref::<sched_context_t>(self.tcbSchedContext).refill_ready()
            );
        }
        let self_ptr = self as *mut tcb_t;
        if self.tcbState.get_tcbQueued() == 0 {
            let dom = self.domain;
            let prio = self.tcbPriority;
            let idx = ready_queues_index(dom, prio);
            let queue = self.get_sched_queue(idx);

            if queue.head == 0 {
                queue.head = self_ptr as usize;
                add_to_bitmap(self.get_cpu(), dom, prio);
            } else {
                let next = queue.tail;
                // unsafe { (*next).tcbSchedNext = self_ptr as usize };
                convert_to_mut_type_ref::<tcb_t>(next).tcbSchedNext = self_ptr as usize;
            }
            self.tcbSchedPrev = queue.tail;
            self.tcbSchedNext = 0;
            queue.tail = self_ptr as usize;
            unsafe {
                ksReadyQueues[idx] = *queue;
            }

            self.tcbState.set_tcbQueued(1);
        }
        #[cfg(feature = "ENABLE_SMP")]
        self.update_queue();
    }

    #[cfg(feature = "ENABLE_SMP")]
    #[inline]
    fn update_queue(&self) {
        use super::scheduler::{ksCurDomain, ksSMP};
        use sel4_common::utils::{convert_to_type_ref, cpu_id};
        unsafe {
            if self.tcbAffinity != cpu_id() && self.domain == ksCurDomain {
                let target_current =
                    convert_to_type_ref::<tcb_t>(ksSMP[self.tcbAffinity].ksCurThread);
                if ksSMP[self.tcbAffinity].ksIdleThread == ksSMP[self.tcbAffinity].ksCurThread
                    || self.tcbPriority > target_current.tcbPriority
                {
                    ksSMP[cpu_id()].ipiReschedulePending |= BIT!(self.tcbAffinity);
                }
            }
        }
    }

    /// Set the VM root of the TCB
    pub fn set_vm_root(&mut self) -> Result<(), lookup_fault> {
        // let threadRoot = &(*getCSpace(thread as usize, tcbVTable)).cap;
        let thread_root = &self.get_cspace(tcbVTable).capability;
        set_vm_root(thread_root)
    }

    #[inline]
    /// Switch to the TCB(set current thread to self)
    pub fn switch_to_this(&mut self) {
        // if hart_id() == 0 {
        //     debug!("switch_to_this: {:#x}", self.get_ptr());
        // }
        let _ = self.set_vm_root();
        self.sched_dequeue();
        set_current_thread(self);
    }

    #[inline]
    /// Get the pointer of the TCB
    /// # Returns
    /// The raw pointer of the TCB
    pub fn get_ptr(&self) -> pptr_t {
        self as *const tcb_t as usize
    }

    #[inline]
    /// Get the mut pointer of the TCB
    /// # Returns
    /// The raw mut pointer of the TCB
    pub fn get_mut_ptr(&mut self) -> pptr_t {
        self as *mut tcb_t as usize
    }

    #[inline]
    /// Look up the slot of the TCB
    /// # Arguments
    /// * `cap_ptr` - The capability pointer to look up
    /// # Returns
    /// The lookup result structure
    pub fn lookup_slot(&mut self, cap_ptr: usize) -> lookupSlot_raw_ret_t {
        let thread_root = &self.get_cspace(tcbCTable).capability;
        let res_ret = resolve_address_bits(&thread_root, cap_ptr, wordBits);
        lookupSlot_raw_ret_t {
            status: res_ret.status,
            slot: res_ret.slot,
        }
    }

    #[inline]
    #[cfg(not(feature = "KERNEL_MCS"))]
    /// Setup the reply master of the TCB
    pub fn setup_reply_master(&mut self) {
        let slot = self.get_cspace_mut_ref(tcbReply);
        if slot.capability.get_tag() == cap_tag::cap_null_cap {
            slot.capability = cap_reply_cap::new(self.get_ptr() as u64, 1, 1).unsplay();
            slot.cteMDBNode = mdb_node::new(0, 1, 1, 0);
        }
    }

    #[inline]
    /// Susupend the TCB, set the state to ThreadStateInactive and dequeue from the scheduling queue
    pub fn suspend(&mut self) {
        if self.get_state() == ThreadState::ThreadStateRunning {
            self.tcbArch.set_register(
                ArchReg::FaultIP,
                self.tcbArch.get_register(ArchReg::FaultIP),
            );
        }
        // set_thread_state(self as *mut Self, ThreadStateInactive);
        // println!("tcb suspend: {:#x}", self.get_ptr());
        set_thread_state(self, ThreadState::ThreadStateInactive);
        self.sched_dequeue();
        #[cfg(feature = "KERNEL_MCS")]
        self.release_remove();
        #[cfg(feature = "KERNEL_MCS")]
        self.sched_context_cancel_yield_to();
    }

    #[inline]
    // void restart(tcb_t *target)
    // {
    //     if (isStopped(target))
    //     {
    //         cancelIPC(target);
    // #ifdef CONFIG_KERNEL_MCS
    //         set_thread_state(target, ThreadState_Restart);
    //         if (sc_sporadic(target->tcbSchedContext) && target->tcbSchedContext != NODE_STATE(ksCurSC))
    //         {
    //             refill_unblock_check(target->tcbSchedContext);
    //         }
    //         sched_context_resume(target->tcbSchedContext);
    //         if (isSchedulable(target))
    //         {
    //             possibleSwitchTo(target);
    //         }
    // #else
    //         setupReplyMaster(target);
    //         set_thread_state(target, ThreadState_Restart);
    //         SCHED_ENQUEUE(target);
    //         possibleSwitchTo(target);
    // #endif
    //     }
    // }
    /// Restart the TCB, set the state to ThreadStateRestart and enqueue to the scheduling queue waiting for reschedule
    pub fn restart(&mut self) {
        if self.is_stopped() {
            #[cfg(feature = "KERNEL_MCS")]
            {
                // MCS
                set_thread_state(self, ThreadState::ThreadStateRestart);
                if let Some(sc) =
                    convert_to_option_mut_type_ref::<sched_context_t>(self.tcbSchedContext)
                {
                    if sc.sc_sporadic() && self.tcbSchedContext != unsafe { ksCurSC } {
                        sc.refill_unblock_check();
                    }
                    sc.sched_context_resume();
                }
                if self.is_schedulable() {
                    possible_switch_to(self);
                }
            }
            #[cfg(not(feature = "KERNEL_MCS"))]
            {
                self.setup_reply_master();
                // set_thread_state(self as *mut Self, ThreadStateRestart);
                set_thread_state(self, ThreadState::ThreadStateRestart);
                self.sched_enqueue();
                possible_switch_to(self);
            }
        }
    }

    #[inline]
    #[cfg(not(feature = "KERNEL_MCS"))]
    /// Setup the caller cap of the TCB
    /// # Arguments
    /// * `sender` - The sender TCB
    /// * `can_grant` - If the cap can be granted
    pub fn setup_caller_cap(&mut self, sender: &mut Self, can_grant: bool) {
        set_thread_state(sender, ThreadState::ThreadStateBlockedOnReply);
        let reply_slot = sender.get_cspace_mut_ref(tcbReply);
        let master_cap = cap::cap_reply_cap(&reply_slot.capability);

        assert_eq!(
            master_cap.clone().unsplay().get_tag(),
            cap_tag::cap_reply_cap
        );
        assert_eq!(master_cap.get_capReplyMaster(), 1);
        assert_eq!(master_cap.get_capReplyCanGrant(), 1);
        assert_eq!(master_cap.get_capTCBPtr() as usize, sender.get_ptr());

        let caller_slot = self.get_cspace_mut_ref(tcbCaller);
        assert_eq!(caller_slot.capability.get_tag(), cap_tag::cap_null_cap);
        cte_insert(
            &cap_reply_cap::new(sender.get_ptr() as u64, can_grant as u64, 0).unsplay(),
            reply_slot,
            caller_slot,
        );
    }

    #[inline]
    #[cfg(not(feature = "KERNEL_MCS"))]
    /// Delete the caller cap of the TCB
    pub fn delete_caller_cap(&mut self) {
        let caller_slot = self.get_cspace_mut_ref(tcbCaller);
        caller_slot.delete_one();
    }

    /// Look up the IPC buffer of the TCB
    /// # Arguments
    /// * `is_receiver` - If the TCB is receiver
    /// # Returns
    /// The IPC buffer of the TCB
    pub fn lookup_ipc_buffer(&mut self, is_receiver: bool) -> Option<&'static seL4_IPCBuffer> {
        let w_buffer_ptr = self.tcbIPCBuffer;
        let buffer_cap = cap::cap_frame_cap(&self.get_cspace(tcbBuffer).capability);
        if unlikely(buffer_cap.clone().unsplay().get_tag() != cap_tag::cap_frame_cap) {
            return None;
        }

        if unlikely(buffer_cap.get_capFIsDevice() != 0) {
            return None;
        }

        let vm_rights: vm_rights_t = unsafe { core::mem::transmute(buffer_cap.get_capFVMRights()) };
        if likely(
            vm_rights == vm_rights_t::VMReadWrite
                || (!is_receiver && vm_rights == vm_rights_t::VMReadOnly),
        ) {
            let base_ptr = buffer_cap.get_capFBasePtr() as usize;
            let page_bits = pageBitsForSize(buffer_cap.get_capFSize() as usize);
            return Some(convert_to_mut_type_ref::<seL4_IPCBuffer>(
                base_ptr + (w_buffer_ptr & MASK!(page_bits)),
            ));
        }
        return None;
    }

    /// Look up the extra caps of the TCB
    /// # Arguments
    /// * `res` - The result array to store the extra caps
    /// # Returns
    /// The result of the lookup represented by seL4_Fault_t
    pub fn lookup_extra_caps(&mut self, res: &mut [pptr_t; seL4_MsgMaxExtraCaps]) -> exception_t {
        let info =
            seL4_MessageInfo::from_word_security(self.tcbArch.get_register(ArchReg::MsgInfo));
        if let Some(buffer) = self.lookup_ipc_buffer(false) {
            let length = info.get_extraCaps();
            let mut i = 0;
            while i < length {
                let cptr = buffer.get_extra_cptr(i as usize);
                let lu_ret = self.lookup_slot(cptr);
                if unlikely(lu_ret.status != exception_t::EXCEPTION_NONE) {
                    unsafe {
                        current_fault =
                            seL4_Fault_CapFault::new(cptr as u64, false as u64).unsplay();
                    }
                    return lu_ret.status;
                }
                res[i as usize] = lu_ret.slot as usize;
                i += 1;
            }
            if i < seL4_MsgMaxExtraCaps as u64 {
                res[i as usize] = 0;
            }
        } else {
            res[0] = 0;
        }
        exception_t::EXCEPTION_NONE
    }

    /// Look up the extra caps of the TCB with IPC buffer
    /// # Arguments
    /// * `res` - The result array to store the extra caps
    /// * `buf` - The IPC buffer to look up
    /// # Returns
    /// The result of the lookup represented by seL4_Fault_t
    pub fn lookup_extra_caps_with_buf(
        &mut self,
        res: &mut [pptr_t; seL4_MsgMaxExtraCaps],
        buf: Option<&seL4_IPCBuffer>,
    ) -> Result<(), seL4_Fault> {
        let info =
            seL4_MessageInfo::from_word_security(self.tcbArch.get_register(ArchReg::MsgInfo));
        if let Some(buffer) = buf {
            let length = info.get_extraCaps();
            let mut i = 0;
            while i < length {
                let cptr = buffer.get_extra_cptr(i as usize);
                let lu_ret = self.lookup_slot(cptr);
                if unlikely(lu_ret.status != exception_t::EXCEPTION_NONE) {
                    return Err(seL4_Fault_CapFault::new(cptr as u64, false as u64).unsplay());
                }
                res[i as usize] = lu_ret.slot as usize;
                i += 1;
            }
            if i < seL4_MsgMaxExtraCaps as u64 {
                res[i as usize] = 0;
            }
        }
        Ok(())
    }

    /// As same as `lookup_ipc_buffer`, but the result is mutable reference
    pub fn lookup_mut_ipc_buffer(
        &mut self,
        is_receiver: bool,
    ) -> Option<&'static mut seL4_IPCBuffer> {
        let w_buffer_ptr = self.tcbIPCBuffer;
        let buffer_cap = cap::cap_frame_cap(&self.get_cspace(tcbBuffer).capability);
        if buffer_cap.clone().unsplay().get_tag() != cap_tag::cap_frame_cap {
            return None;
        }

        let vm_rights: vm_rights_t = unsafe { core::mem::transmute(buffer_cap.get_capFVMRights()) };
        if vm_rights == vm_rights_t::VMReadWrite
            || (!is_receiver && vm_rights == vm_rights_t::VMReadOnly)
        {
            let base_ptr = buffer_cap.get_capFBasePtr() as usize;
            let page_bits = pageBitsForSize(buffer_cap.get_capFSize() as usize);
            return Some(convert_to_mut_type_ref::<seL4_IPCBuffer>(
                base_ptr + (w_buffer_ptr & MASK!(page_bits)),
            ));
        }
        return None;
    }

    #[inline]
    /// Set the message info register of the TCB
    /// # Arguments
    /// * `offset` - The offset of the message info register, if the offset is larger than n_msgRegisters, set to the IPC buffer
    /// * `reg` - The value to set
    /// # Returns
    /// The next offset
    pub fn set_mr(&mut self, offset: usize, reg: usize) -> usize {
        if offset >= msgRegisterNum {
            if let Some(ipc_buffer) = self.lookup_mut_ipc_buffer(true) {
                ipc_buffer.msg[offset] = reg;
                return offset + 1;
            } else {
                return msgRegisterNum;
            }
        } else {
            self.tcbArch.set_register(ArchReg::Msg(offset), reg);
            return offset + 1;
        }
    }

    /// Set the lookup fault to the msg registers of the TCB
    /// # Arguments
    /// * `offset` - The offset of the lookup fault
    /// * `fault` - The lookup fault to set
    /// # Returns
    /// The next offset
    pub fn set_lookup_fault_mrs(&mut self, offset: usize, fault: &lookup_fault) -> usize {
        let luf_type = fault.get_tag() as usize;
        let i = self.set_mr(offset, luf_type + 1);
        if offset == seL4_CapFault_LookupFailureType {
            assert_eq!(offset + 1, seL4_CapFault_BitsLeft);
            assert_eq!(offset + 2, seL4_CapFault_DepthMismatch_BitsFound);
            assert_eq!(offset + 2, seL4_CapFault_GuardMismatch_GuardFound);
            assert_eq!(offset + 3, seL4_CapFault_GuardMismatch_BitsFound);
        } else {
            assert_eq!(offset, 1);
        }
        match fault.clone().splay() {
            lookup_fault_Splayed::invalid_root(_) => i,
            lookup_fault_Splayed::missing_capability(data) => {
                self.set_mr(offset + 1, data.get_bitsLeft() as usize)
            }
            lookup_fault_Splayed::depth_mismatch(data) => {
                self.set_mr(offset + 1, data.get_bitsLeft() as usize);
                self.set_mr(offset + 2, data.get_bitsFound() as usize)
            }
            lookup_fault_Splayed::guard_mismatch(data) => {
                self.set_mr(offset + 1, data.get_bitsLeft() as usize);
                self.set_mr(offset + 2, data.get_guardFound() as usize);
                self.set_mr(offset + 3, data.get_bitsFound() as usize)
            }
        }
    }

    /// Get the receive slot of the TCB
    /// # Returns
    /// The mutable ref of receive slot of the TCB
    pub fn get_receive_slot(&mut self) -> Option<&'static mut cte_t> {
        if let Some(buffer) = self.lookup_ipc_buffer(true) {
            let cptr = buffer.receiveCNode;
            let lu_ret = self.lookup_slot(cptr);
            if lu_ret.status != exception_t::EXCEPTION_NONE {
                return None;
            }
            let cnode_cap = unsafe { &(*lu_ret.slot).capability };
            let lus_ret = resolve_address_bits(cnode_cap, buffer.receiveIndex, buffer.receiveDepth);
            if unlikely(lus_ret.status != exception_t::EXCEPTION_NONE || lus_ret.bitsRemaining != 0)
            {
                return None;
            }
            return Some(convert_to_mut_type_ref::<cte_t>(lus_ret.slot as usize));
        }
        return None;
    }

    #[inline]
    /// Copy the message registers and ipc buffer(if valid) of the TCB to the receiver
    /// # Arguments
    /// * `receiver` - The receiver TCB
    /// * `length` - The length of the message registers to copy
    /// # Returns
    /// The number of registers(contains ipc buffer) copied
    pub fn copy_mrs(&mut self, receiver: &mut tcb_t, length: usize) -> usize {
        let mut i = 0;
        while i < length && i < msgRegisterNum {
            receiver
                .tcbArch
                .set_register(ArchReg::Msg(i), self.tcbArch.get_register(ArchReg::Msg(i)));
            i += 1;
        }
        if let (Some(send_buffer), Some(recv_buffer)) = (
            self.lookup_ipc_buffer(false),
            receiver.lookup_mut_ipc_buffer(true),
        ) {
            unsafe {
                let recv_ptr = recv_buffer as *mut seL4_IPCBuffer as *mut usize;
                let send_ptr = send_buffer as *const seL4_IPCBuffer as *const usize;
                while i < length {
                    *(recv_ptr.add(i + 1)) = *(send_ptr.add(i + 1));
                    i += 1;
                }
            }
        }
        i
    }

    #[inline]
    /// Copy the falut messages and ipc buffer(if valid) of the TCB to the receiver
    /// # Arguments
    /// * `receiver` - The receiver TCB
    /// * `id` - The fault message id
    /// * `length` - The length of the message registers to copy
    pub fn copy_fault_mrs(&self, receiver: &mut Self, id: usize, length: usize) {
        let len = core::cmp::min(length, msgRegisterNum);

        for i in 0..len {
            receiver.tcbArch.set_register(
                ArchReg::Msg(i),
                self.tcbArch.get_register(ArchReg::FaultMessage(id, i)),
            );
        }
        if let Some(buffer) = receiver.lookup_mut_ipc_buffer(true) {
            for i in len..length {
                buffer.msg[i] = self.tcbArch.get_register(ArchReg::FaultMessage(id, i));
            }
        }
    }

    #[inline]
    /// Copy the falut messages for reply and ipc buffer(if valid) of the TCB to the receiver for reply
    /// # Arguments
    /// * `receiver` - The receiver TCB
    /// * `id` - The fault message id
    /// * `length` - The length of the message registers to copy
    pub fn copy_fault_mrs_for_reply(&mut self, receiver: &mut Self, id: usize, length: usize) {
        let len = core::cmp::min(length, msgRegisterNum);

        for i in 0..len {
            receiver.tcbArch.set_register(
                ArchReg::FaultMessage(id, i),
                self.tcbArch.get_register(ArchReg::Msg(i)),
            );
        }

        if let Some(buffer) = self.lookup_ipc_buffer(false) {
            for i in len..length {
                receiver
                    .tcbArch
                    .set_register(ArchReg::FaultMessage(id, i), buffer.msg[i])
            }
        }
    }

    #[inline]
    /// Copy the syscall fault messages of the TCB to the receiver
    pub fn copy_syscall_fault_mrs(&self, receiver: &mut Self) {
        self.copy_fault_mrs(receiver, MessageID_Syscall, n_syscallMessage)
    }

    #[inline]
    /// Copy the exception fault messages of the TCB to the receiver
    pub fn copy_exeception_fault_mrs(&self, receiver: &mut Self) {
        self.copy_fault_mrs(receiver, MessageID_Exception, n_exceptionMessage)
    }

    #[inline]
    /// Set the fault message registers of the TCB to the receiver
    /// # Arguments
    /// * `receiver` - The receiver TCB
    pub fn set_fault_mrs(&self, receiver: &mut Self) -> usize {
        match self.tcbFault.get_tag() {
            seL4_Fault_tag::seL4_Fault_CapFault => {
                receiver.set_mr(
                    seL4_CapFault_IP,
                    self.tcbArch.get_register(ArchReg::FaultIP),
                );
                receiver.set_mr(
                    seL4_CapFault_Addr,
                    seL4_Fault::seL4_Fault_CapFault(&self.tcbFault).get_address() as usize,
                );
                receiver.set_mr(
                    seL4_CapFault_InRecvPhase,
                    seL4_Fault::seL4_Fault_CapFault(&self.tcbFault).get_inReceivePhase() as usize,
                );
                receiver
                    .set_lookup_fault_mrs(seL4_CapFault_LookupFailureType, &self.tcbLookupFailure)
            }
            seL4_Fault_tag::seL4_Fault_UnknownSyscall => {
                self.copy_syscall_fault_mrs(receiver);
                receiver.set_mr(
                    n_syscallMessage,
                    seL4_Fault::seL4_Fault_UnknownSyscall(&self.tcbFault).get_syscallNumber()
                        as usize,
                )
            }
            seL4_Fault_tag::seL4_Fault_UserException => {
                self.copy_exeception_fault_mrs(receiver);
                receiver.set_mr(
                    n_exceptionMessage,
                    seL4_Fault::seL4_Fault_UserException(&self.tcbFault).get_number() as usize,
                );
                receiver.set_mr(
                    n_exceptionMessage + 1,
                    seL4_Fault::seL4_Fault_UserException(&self.tcbFault).get_code() as usize,
                )
            }
            seL4_Fault_tag::seL4_Fault_VMFault => {
                receiver.set_mr(seL4_VMFault_IP, self.tcbArch.get_register(ArchReg::FaultIP));
                receiver.set_mr(
                    seL4_VMFault_Addr,
                    seL4_Fault::seL4_Fault_VMFault(&self.tcbFault).get_address() as usize,
                );
                receiver.set_mr(
                    seL4_VMFault_PrefetchFault,
                    seL4_Fault::seL4_Fault_VMFault(&self.tcbFault).get_instructionFault() as usize,
                );
                receiver.set_mr(
                    seL4_VMFault_FSR,
                    seL4_Fault::seL4_Fault_VMFault(&self.tcbFault).get_FSR() as usize,
                )
            }
            _ => {
                panic!("invalid fault")
            }
        }
    }

    /// Set the thread state
    #[inline]
    pub fn set_state(&mut self, state: ThreadState) {
        self.tcbState.set_tsType(state as u64);
        schedule_tcb(self);
    }
    pub fn debug_append(&mut self) {}
    pub fn debug_remove(&mut self) {}

    #[inline]
    #[cfg(feature = "KERNEL_MCS")]
    pub fn Ready_Time(&self) -> ticks_t {
        unsafe {
            (*convert_to_mut_type_ref::<sched_context_t>(self.tcbSchedContext).refill_head()).rTime
        }
    }
    #[inline]
    #[cfg(feature = "KERNEL_MCS")]
    pub fn time_after(&self, new_time: ticks_t) -> bool {
        new_time >= self.Ready_Time()
    }

    #[inline]
    pub fn queue_insert(&mut self, tcb_after: &mut tcb_t) {
        let before = tcb_after.tcbSchedPrev;
        assert!(before != 0);
        assert!(before != tcb_after.get_ptr());

        self.tcbSchedPrev = before;
        self.tcbSchedNext = tcb_after.get_ptr();

        tcb_after.tcbSchedPrev = self.get_ptr();
        convert_to_mut_type_ref::<tcb_t>(before).tcbSchedNext = self.get_ptr();
    }

    #[inline]
    #[cfg(feature = "KERNEL_MCS")]
    pub fn release_remove(&mut self) {
        use crate::ksReprogram;

        unsafe {
            if likely(self.tcbState.get_tcbInReleaseQueue() != 0) {
                let mut queue = ksReleaseQueue;

                if queue.head == self.get_ptr() {
                    ksReprogram = true;
                }
                queue.remove(self);
                ksReleaseQueue = queue;

                self.tcbState.set_tcbInReleaseQueue(0);
            }
        }
    }
    #[inline]
    #[cfg(feature = "KERNEL_MCS")]
    pub fn release_enqueue(&mut self) {
        use crate::ksReprogram;

        assert!(self.tcbState.get_tcbInReleaseQueue() == 0);
        assert!(self.tcbState.get_tcbQueued() == 0);

        unsafe {
            let new_time = self.Ready_Time();
            let mut queue = ksReleaseQueue;

            if queue.empty() || new_time < convert_to_mut_type_ref::<tcb_t>(queue.head).Ready_Time()
            {
                queue.prepend(self);
                ksReleaseQueue = queue;
                ksReprogram = true;
            } else {
                if convert_to_mut_type_ref::<tcb_t>(queue.tail).Ready_Time() < new_time {
                    queue.append(self);
                    ksReleaseQueue = queue;
                } else {
                    let after =
                        find_time_after(convert_to_mut_type_ref::<tcb_t>(queue.head), new_time);
                    self.queue_insert(convert_to_mut_type_ref(after as usize));
                }
            }
        }

        self.tcbState.set_tcbInReleaseQueue(1);
    }
    #[inline]
    #[cfg(feature = "KERNEL_MCS")]
    pub fn sched_context_cancel_yield_to(&mut self) {
        if self.get_ptr() != 0 && self.tcbYieldTo != 0 {
            convert_to_mut_type_ref::<sched_context_t>(self.tcbYieldTo).scYieldFrom = 0;
            self.tcbYieldTo = 0;
        }
    }
    #[inline]
    #[cfg(feature = "KERNEL_MCS")]
    pub fn schedContext_completeYieldTo(&mut self) {
        if self.get_ptr() != 0 && self.tcbYieldTo != 0 {
            convert_to_mut_type_ref::<sched_context_t>(self.tcbYieldTo).set_consumed();
            self.sched_context_cancel_yield_to();
        }
    }
    #[inline]
    #[cfg(feature = "KERNEL_MCS")]
    pub fn valid_timeout_handler(&mut self) -> bool {
        let cte = self.get_cspace(tcbTimeoutHandler);
        cte.capability.get_tag() == cap_tag::cap_endpoint_cap
    }
}

#[inline]
/// Set the thread state of the TCB
/// # Arguments
/// * `tcb` - The TCB to set
/// * `state` - The state
pub fn set_thread_state(tcb: &mut tcb_t, state: ThreadState) {
    tcb.tcbState.set_tsType(state as u64);
    schedule_tcb(tcb);
}
#[inline]
#[cfg(feature = "KERNEL_MCS")]
pub fn find_time_after(tcb: &mut tcb_t, new_time: ticks_t) -> *mut tcb_t {
    let mut after = tcb;
    while after.time_after(new_time) {
        if after.tcbSchedContext != 0 {
            after = convert_to_mut_type_ref::<tcb_t>(after.tcbSchedContext)
        } else {
            // we do not check the ptr is 0 in time after, but do it here
            break;
        }
    }
    return after;
}

#[cfg(feature = "KERNEL_MCS")]
pub fn tcb_release_dequeue() -> *mut tcb_t {
    use crate::ksReprogram;

    unsafe {
        assert!(ksReleaseQueue.head != 0);
        assert!(convert_to_mut_type_ref::<tcb_t>(ksReleaseQueue.head).tcbSchedPrev == 0);

        let awakened = convert_to_mut_type_ref::<tcb_t>(ksReleaseQueue.head);
        assert!(awakened.get_ptr() != get_currenct_thread().get_ptr());

        awakened.release_remove();
        ksReprogram = true;

        return awakened;
    }
}
#[cfg(feature = "KERNEL_MCS")]
pub fn reply_remove_tcb(tcb: &mut tcb_t) {
    use sel4_common::structures_gen::call_stack;

    use crate::reply::reply_t;
    assert!(tcb.tcbState.get_tsType() == ThreadState::ThreadStateBlockedOnReply as u64);
    let reply = convert_to_mut_type_ref::<reply_t>(tcb.tcbState.get_replyObject() as usize);

    let next_ptr = reply.replyNext.get_callStackPtr() as usize;
    let prev_ptr = reply.replyPrev.get_callStackPtr() as usize;

    if next_ptr != 0 {
        if reply.replyNext.get_isHead() != 0 {
            convert_to_mut_type_ref::<sched_context_t>(next_ptr).scReply = 0;
        } else {
            convert_to_mut_type_ref::<reply_t>(next_ptr).replyPrev = call_stack::new(0, 0);
        }
    }

    if prev_ptr != 0 {
        convert_to_mut_type_ref::<reply_t>(prev_ptr).replyNext = call_stack::new(0, 0);
    }

    reply.replyPrev = call_stack::new(0, 0);
    reply.replyNext = call_stack::new(0, 0);
    reply.unlink(tcb);
}
