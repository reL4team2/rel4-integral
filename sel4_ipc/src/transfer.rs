use core::intrinsics::likely;
use core::intrinsics::unlikely;

use super::endpoint::*;
use super::notification::*;

use sel4_common::arch::ArchReg;
#[cfg(feature = "kernel_mcs")]
use sel4_common::arch::N_TIMEOUT_MESSAGE;
use sel4_common::arch::{N_EXCEPTON_MESSAGE, N_SYSCALL_MESSAGE};
use sel4_common::fault::*;
use sel4_common::message_info::seL4_MessageInfo_func;
use sel4_common::sel4_config::*;
use sel4_common::shared_types_bf_gen::seL4_MessageInfo;
use sel4_common::structures::*;
use sel4_common::structures_gen::{
    cap, cap_tag, endpoint, notification, seL4_Fault, seL4_Fault_NullFault, seL4_Fault_tag,
};
use sel4_common::utils::*;
use sel4_cspace::interface::*;
use sel4_task::{possible_switch_to, set_thread_state, tcb_t, ThreadState};
#[cfg(feature = "kernel_mcs")]
use sel4_task::{reply::reply_t, reply_remove_tcb, sched_context::sched_context_t};
use sel4_vspace::pptr_t;

/// The trait for IPC transfer, please see doc.md for more details
pub trait Transfer {
    fn cancel_ipc(&mut self);

    fn set_transfer_caps(
        &mut self,
        endpoint: Option<&endpoint>,
        info: &mut seL4_MessageInfo,
        current_extra_caps: &[pptr_t; SEL4_MSG_MAX_EXTRA_CAPS],
    );

    fn set_transfer_caps_with_buf(
        &mut self,
        endpoint: Option<&endpoint>,
        info: &mut seL4_MessageInfo,
        current_extra_caps: &[pptr_t; SEL4_MSG_MAX_EXTRA_CAPS],
        ipc_buffer: Option<&mut seL4_IPCBuffer>,
    );

    fn do_fault_transfer(&self, receiver: &mut tcb_t, badge: usize);

    fn do_normal_transfer(
        &mut self,
        receiver: &mut tcb_t,
        endpoint: Option<&endpoint>,
        badge: usize,
        can_grant: bool,
    );

    fn do_fault_reply_transfer(&mut self, receiver: &mut tcb_t) -> bool;

    fn complete_signal(&mut self) -> bool;

    fn do_ipc_transfer(
        &mut self,
        receiver: &mut tcb_t,
        endpoint: Option<&endpoint>,
        badge: usize,
        grant: bool,
    );
    #[cfg(feature = "kernel_mcs")]
    fn do_reply(&mut self, reply: &mut reply_t, grant: bool);
    #[cfg(not(feature = "kernel_mcs"))]
    fn do_reply(&mut self, receiver: &mut tcb_t, slot: &mut cte_t, grant: bool);
}

impl Transfer for tcb_t {
    fn cancel_ipc(&mut self) {
        let state = &self.tcbState;
        #[cfg(feature = "kernel_mcs")]
        {
            seL4_Fault_NullFault::new();
        }
        match self.get_state() {
            ThreadState::ThreadStateBlockedOnSend | ThreadState::ThreadStateBlockedOnReceive => {
                let ep = convert_to_mut_type_ref::<endpoint>(state.get_blockingObject() as usize);
                assert_ne!(ep.get_ep_state(), EPState::Idle);
                ep.cancel_ipc(self);
            }
            ThreadState::ThreadStateBlockedOnNotification => {
                let ntfn =
                    convert_to_mut_type_ref::<notification>(state.get_blockingObject() as usize);
                ntfn.cancel_signal(self);
            }

            ThreadState::ThreadStateBlockedOnReply => {
                #[cfg(feature = "kernel_mcs")]
                {
                    reply_remove_tcb(self);
                }
                #[cfg(not(feature = "kernel_mcs"))]
                {
                    self.tcbFault = seL4_Fault_NullFault::new().unsplay();
                    let slot = self.get_cspace(TCB_REPLY);
                    let caller_slot_ptr = slot.cteMDBNode.get_mdbNext() as usize;
                    if caller_slot_ptr != 0 {
                        convert_to_mut_type_ref::<cte_t>(caller_slot_ptr).delete_one()
                    }
                }
            }
            _ => {}
        }
    }

    fn set_transfer_caps(
        &mut self,
        ep: Option<&endpoint>,
        info: &mut seL4_MessageInfo,
        current_extra_caps: &[pptr_t; SEL4_MSG_MAX_EXTRA_CAPS],
    ) {
        info.set_extraCaps(0);
        info.set_capsUnwrapped(0);
        let ipc_buffer = self.lookup_mut_ipc_buffer(true);
        if current_extra_caps[0] as usize == 0 || ipc_buffer.is_none() {
            return;
        }
        let buffer = ipc_buffer.unwrap();
        let mut dest_slot = self.get_receive_slot();
        let mut i = 0;
        while i < SEL4_MSG_MAX_EXTRA_CAPS && current_extra_caps[i] as usize != 0 {
            let slot = convert_to_mut_type_ref::<cte_t>(current_extra_caps[i]);
            let capability_cpy = &slot.capability.clone();
            if capability_cpy.get_tag() == cap_tag::cap_endpoint_cap
                && ep.is_some()
                && cap::cap_endpoint_cap(capability_cpy).get_capEPPtr() as usize
                    == ep.unwrap().get_ptr()
            {
                buffer.caps_or_badges[i] =
                    cap::cap_endpoint_cap(capability_cpy).get_capEPBadge() as usize;
                info.set_capsUnwrapped(info.get_capsUnwrapped() | (1 << i));
            } else {
                if dest_slot.is_none() {
                    break;
                } else {
                    let dest = dest_slot.take();
                    let dc_ret = slot.derive_cap(&capability_cpy);
                    if dc_ret.status != exception_t::EXCEPTION_NONE
                        || dc_ret.capability.get_tag() == cap_tag::cap_null_cap
                    {
                        break;
                    }
                    cte_insert(&dc_ret.capability, slot, dest.unwrap());
                    dest_slot = None;
                }
            }
            i += 1;
        }
        info.set_extraCaps(i as u64);
    }

    fn set_transfer_caps_with_buf(
        &mut self,
        ep: Option<&endpoint>,
        info: &mut seL4_MessageInfo,
        current_extra_caps: &[pptr_t; SEL4_MSG_MAX_EXTRA_CAPS],
        ipc_buffer: Option<&mut seL4_IPCBuffer>,
    ) {
        info.set_extraCaps(0);
        info.set_capsUnwrapped(0);
        // let ipc_buffer = self.lookup_mut_ipc_buffer(true);
        if likely(current_extra_caps[0] as usize == 0 || ipc_buffer.is_none()) {
            return;
        }
        let buffer = ipc_buffer.unwrap();
        let mut dest_slot = self.get_receive_slot();
        let mut i = 0;
        while i < SEL4_MSG_MAX_EXTRA_CAPS && current_extra_caps[i] as usize != 0 {
            let slot = convert_to_mut_type_ref::<cte_t>(current_extra_caps[i]);
            let capability_cpy = &slot.capability.clone();
            if capability_cpy.get_tag() == cap_tag::cap_endpoint_cap
                && ep.is_some()
                && cap::cap_endpoint_cap(capability_cpy).get_capEPPtr() as usize
                    == ep.unwrap().get_ptr()
            {
                buffer.caps_or_badges[i] =
                    cap::cap_endpoint_cap(capability_cpy).get_capEPBadge() as usize;
                info.set_capsUnwrapped(info.get_capsUnwrapped() | (1 << i));
            } else {
                if dest_slot.is_none() {
                    break;
                } else {
                    let dest = dest_slot.take();
                    let dc_ret = slot.derive_cap(&capability_cpy);
                    if dc_ret.status != exception_t::EXCEPTION_NONE
                        || dc_ret.capability.get_tag() == cap_tag::cap_null_cap
                    {
                        break;
                    }
                    cte_insert(&dc_ret.capability, slot, dest.unwrap());
                }
            }
            i += 1;
        }
        info.set_extraCaps(i as u64);
    }

    fn do_fault_transfer(&self, receiver: &mut tcb_t, badge: usize) {
        let sent = match self.tcbFault.get_tag() {
            seL4_Fault_tag::seL4_Fault_CapFault => {
                receiver.set_mr(CAP_FAULT_IP, self.tcbArch.get_register(ArchReg::FAULT_IP));
                receiver.set_mr(
                    CAP_FAULT_ADDR,
                    seL4_Fault::seL4_Fault_CapFault(&self.tcbFault).get_address() as usize,
                );
                receiver.set_mr(
                    CAP_FAULT_IN_RECV_PHASE,
                    seL4_Fault::seL4_Fault_CapFault(&self.tcbFault).get_inReceivePhase() as usize,
                );
                receiver.set_lookup_fault_mrs(CAP_FAULT_LOOKUP_FAILURE_TYPE, &self.tcbLookupFailure)
            }
            seL4_Fault_tag::seL4_Fault_UnknownSyscall => {
                self.copy_syscall_fault_mrs(receiver);
                receiver.set_mr(
                    N_SYSCALL_MESSAGE,
                    seL4_Fault::seL4_Fault_UnknownSyscall(&self.tcbFault).get_syscallNumber()
                        as usize,
                )
            }
            seL4_Fault_tag::seL4_Fault_UserException => {
                self.copy_exeception_fault_mrs(receiver);
                receiver.set_mr(
                    N_EXCEPTON_MESSAGE,
                    seL4_Fault::seL4_Fault_UserException(&self.tcbFault).get_number() as usize,
                );
                receiver.set_mr(
                    N_EXCEPTON_MESSAGE + 1,
                    seL4_Fault::seL4_Fault_UserException(&self.tcbFault).get_code() as usize,
                )
            }
            seL4_Fault_tag::seL4_Fault_VMFault => {
                receiver.set_mr(VM_FAULT_IP, self.tcbArch.get_register(ArchReg::FAULT_IP));
                receiver.set_mr(
                    VM_FAULT_ADDR,
                    seL4_Fault::seL4_Fault_VMFault(&self.tcbFault).get_address() as usize,
                );
                receiver.set_mr(
                    VM_FAULT_PREFETCH_FAULT,
                    seL4_Fault::seL4_Fault_VMFault(&self.tcbFault).get_instructionFault() as usize,
                );
                receiver.set_mr(
                    VM_FAULT_FSR,
                    seL4_Fault::seL4_Fault_VMFault(&self.tcbFault).get_FSR() as usize,
                )
            }
            #[cfg(feature = "kernel_mcs")]
            seL4_Fault_tag::seL4_Fault_Timeout => {
                let len = receiver.set_mr(
                    TIMEOUT_DATA,
                    seL4_Fault::seL4_Fault_Timeout(&self.tcbFault).get_badge() as usize,
                );
                if let Some(sc) =
                    convert_to_option_mut_type_ref::<sched_context_t>(self.tcbSchedContext)
                {
                    let consumed = sc.sched_context_update_consumed();
                    receiver.set_mr(len, consumed)
                } else {
                    len
                }
            }
            _ => {
                panic!("invalid fault")
            }
        };
        let msg_info = seL4_MessageInfo::new(self.tcbFault.get_tag() as u64, 0, 0, sent as u64);
        receiver
            .tcbArch
            .set_register(ArchReg::MsgInfo, msg_info.to_word());
        receiver.tcbArch.set_register(ArchReg::Badge, badge);
    }

    fn do_normal_transfer(
        &mut self,
        receiver: &mut tcb_t,
        ep: Option<&endpoint>,
        badge: usize,
        can_grant: bool,
    ) {
        let mut tag =
            seL4_MessageInfo::from_word_security(self.tcbArch.get_register(ArchReg::MsgInfo));
        let mut current_extra_caps = [0; SEL4_MSG_MAX_EXTRA_CAPS];
        if can_grant {
            let status = self.lookup_extra_caps(&mut current_extra_caps);
            if unlikely(status != exception_t::EXCEPTION_NONE) {
                current_extra_caps[0] = 0;
            }
        } else {
            current_extra_caps[0] = 0;
        }
        let msg_transferred = self.copy_mrs(receiver, tag.get_length() as usize);
        receiver.set_transfer_caps(ep, &mut tag, &current_extra_caps);
        tag.set_length(msg_transferred as u64);
        receiver
            .tcbArch
            .set_register(ArchReg::MsgInfo, tag.to_word());
        receiver.tcbArch.set_register(ArchReg::Badge, badge);
    }

    fn do_fault_reply_transfer(&mut self, receiver: &mut tcb_t) -> bool {
        let tag = seL4_MessageInfo::from_word_security(self.tcbArch.get_register(ArchReg::MsgInfo));
        let label = tag.get_label() as usize;
        let length = tag.get_length() as usize;
        match receiver.tcbFault.get_tag() {
            seL4_Fault_tag::seL4_Fault_UnknownSyscall => {
                self.copy_fault_mrs_for_reply(
                    receiver,
                    MESSAGE_ID_SYSCALL,
                    core::cmp::min(length, N_SYSCALL_MESSAGE),
                );
                return label as usize == 0;
            }
            seL4_Fault_tag::seL4_Fault_UserException => {
                self.copy_fault_mrs_for_reply(
                    receiver,
                    MESSAGE_ID_EXCEPTION,
                    core::cmp::min(length, N_EXCEPTON_MESSAGE),
                );
                return label as usize == 0;
            }
            #[cfg(feature = "kernel_mcs")]
            seL4_Fault_tag::seL4_Fault_Timeout => {
                self.copy_fault_mrs_for_reply(
                    receiver,
                    MESSAGE_ID_TIMEOUT_REPLY,
                    core::cmp::min(length, N_TIMEOUT_MESSAGE),
                );
                return label as usize == 0;
            }
            _ => true,
        }
    }

    fn complete_signal(&mut self) -> bool {
        if let Some(ntfn) =
            convert_to_option_mut_type_ref::<notification>(self.tcbBoundNotification)
        {
            if likely(ntfn.get_ntfn_state() == NtfnState::Active) {
                self.tcbArch
                    .set_register(ArchReg::Badge, ntfn.get_ntfnMsgIdentifier() as usize);
                ntfn.set_state(NtfnState::Idle as u64);
                #[cfg(feature = "kernel_mcs")]
                {
                    maybe_donate_sched_context(self, ntfn);
                    if let Some(tcbsc) =
                        convert_to_option_mut_type_ref::<sched_context_t>(self.tcbSchedContext)
                    {
                        if tcbsc.sc_sporadic() {
                            if self.tcbSchedContext == ntfn.get_ntfnSchedContext() as usize
                                && !tcbsc.is_current()
                            {
                                tcbsc.refill_unblock_check();
                            }
                        }
                    }
                }
                return true;
            }
        }
        false
    }

    fn do_ipc_transfer(
        &mut self,
        receiver: &mut tcb_t,
        ep: Option<&endpoint>,
        badge: usize,
        grant: bool,
    ) {
        if likely(self.tcbFault.get_tag() == seL4_Fault_tag::seL4_Fault_NullFault) {
            self.do_normal_transfer(receiver, ep, badge, grant)
        } else {
            self.do_fault_transfer(receiver, badge)
        }
    }
    #[cfg(feature = "kernel_mcs")]
    fn do_reply(&mut self, reply: &mut reply_t, grant: bool) {
        use sel4_common::{ffi::current_fault, structures_gen::seL4_Fault_Timeout};
        use sel4_task::handleTimeout;

        if reply.replyTCB == 0
            || convert_to_mut_type_ref::<tcb_t>(reply.replyTCB)
                .tcbState
                .get_tsType()
                != ThreadState::ThreadStateBlockedOnReply as u64
        {
            /* nothing to do */
            return;
        }

        let receiver = convert_to_mut_type_ref::<tcb_t>(reply.replyTCB);
        reply.remove(receiver);
        assert!(receiver.tcbState.get_replyObject() == 0);
        assert!(reply.replyTCB == 0);

        if let Some(sc) =
            convert_to_option_mut_type_ref::<sched_context_t>(receiver.tcbSchedContext)
        {
            if sc.sc_sporadic() && !sc.is_current() {
                sc.refill_unblock_check();
            }
        }

        let fault_type = receiver.tcbFault.get_tag();
        if likely(fault_type == seL4_Fault_tag::seL4_Fault_NullFault) {
            self.do_ipc_transfer(receiver, None, 0, grant);
            set_thread_state(receiver, ThreadState::ThreadStateRunning);
        } else {
            let restart = self.do_fault_reply_transfer(receiver);
            receiver.tcbFault = seL4_Fault_NullFault::new().unsplay();
            if restart {
                set_thread_state(receiver, ThreadState::ThreadStateRestart);
            } else {
                set_thread_state(receiver, ThreadState::ThreadStateInactive);
            }
        }
        if receiver.tcbSchedContext != 0 && receiver.is_runnable() {
            let sc = convert_to_mut_type_ref::<sched_context_t>(receiver.tcbSchedContext);
            if sc.refill_ready() && sc.refill_sufficient(0) {
                possible_switch_to(receiver);
            } else {
                if receiver.valid_timeout_handler()
                    && fault_type != seL4_Fault_tag::seL4_Fault_Timeout as u64
                {
                    unsafe {
                        current_fault = seL4_Fault_Timeout::new(sc.scBadge as u64).unsplay();
                        handleTimeout(receiver)
                    };
                } else {
                    sc.postpone();
                }
            }
        }
    }
    #[cfg(not(feature = "kernel_mcs"))]
    fn do_reply(&mut self, receiver: &mut tcb_t, slot: &mut cte_t, grant: bool) {
        assert_eq!(receiver.get_state(), ThreadState::ThreadStateBlockedOnReply);
        let fault_type = receiver.tcbFault.get_tag();
        if likely(fault_type == seL4_Fault_tag::seL4_Fault_NullFault) {
            self.do_ipc_transfer(receiver, None, 0, grant);
            slot.delete_one();
            set_thread_state(receiver, ThreadState::ThreadStateRunning);
            possible_switch_to(receiver);
        } else {
            slot.delete_one();
            let restart = self.do_fault_reply_transfer(receiver);
            receiver.tcbFault = seL4_Fault_NullFault::new().unsplay();
            if restart {
                set_thread_state(receiver, ThreadState::ThreadStateRestart);
                possible_switch_to(receiver);
            } else {
                set_thread_state(receiver, ThreadState::ThreadStateInactive);
            }
        }
    }
}
