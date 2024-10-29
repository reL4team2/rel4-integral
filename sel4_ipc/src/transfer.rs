use core::intrinsics::likely;

use super::endpoint::*;
use super::notification::*;
use sel4_common::structures_gen::endpoint;
use sel4_common::structures_gen::notification;

use sel4_common::arch::ArchReg;
use sel4_common::arch::{n_exceptionMessage, n_syscallMessage};
use sel4_common::fault::*;
use sel4_common::message_info::seL4_MessageInfo_func;
use sel4_common::sel4_config::*;
use sel4_common::shared_types_bf_gen::seL4_MessageInfo;
use sel4_common::structures::*;
use sel4_common::structures_gen::cap;
use sel4_common::structures_gen::cap_tag;
use sel4_common::structures_gen::seL4_Fault;
use sel4_common::structures_gen::seL4_Fault_NullFault;
use sel4_common::structures_gen::seL4_Fault_tag;
use sel4_common::utils::*;
use sel4_cspace::interface::*;
use sel4_task::{possible_switch_to, set_thread_state, tcb_t, ThreadState};
use sel4_vspace::pptr_t;

/// The trait for IPC transfer, please see doc.md for more details
pub trait Transfer {
    fn cancel_ipc(&mut self);

    fn set_transfer_caps(
        &mut self,
        endpoint: Option<&endpoint>,
        info: &mut seL4_MessageInfo,
        current_extra_caps: &[pptr_t; seL4_MsgMaxExtraCaps],
    );

    fn set_transfer_caps_with_buf(
        &mut self,
        endpoint: Option<&endpoint>,
        info: &mut seL4_MessageInfo,
        current_extra_caps: &[pptr_t; seL4_MsgMaxExtraCaps],
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

    fn do_reply(&mut self, receiver: &mut tcb_t, slot: &mut cte_t, grant: bool);
}

impl Transfer for tcb_t {
    fn cancel_ipc(&mut self) {
        let state = &self.tcbState;
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
                self.tcbFault = seL4_Fault_NullFault::new().unsplay();
                let slot = self.get_cspace(tcbReply);
                let caller_slot_ptr = slot.cteMDBNode.get_mdbNext() as usize;
                if caller_slot_ptr != 0 {
                    convert_to_mut_type_ref::<cte_t>(caller_slot_ptr).delete_one()
                }
            }
            _ => {}
        }
    }

    fn set_transfer_caps(
        &mut self,
        ep: Option<&endpoint>,
        info: &mut seL4_MessageInfo,
        current_extra_caps: &[pptr_t; seL4_MsgMaxExtraCaps],
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
        while i < seL4_MsgMaxExtraCaps && current_extra_caps[i] as usize != 0 {
            let slot = convert_to_mut_type_ref::<cte_t>(current_extra_caps[i]);
            let capability_cpy = &slot.capability.clone();
            let capability = cap::cap_endpoint_cap(capability_cpy);
            if capability.clone().unsplay().get_tag() == cap_tag::cap_endpoint_cap
                && ep.is_some()
                && capability.get_capEPPtr() as usize == ep.unwrap().get_ptr()
            {
                buffer.caps_or_badges[i] = capability.get_capEPBadge() as usize;
                info.set_capsUnwrapped(info.get_capsUnwrapped() | (1 << i));
            } else {
                if dest_slot.is_none() {
                    break;
                } else {
                    let dest = dest_slot.take();
                    let dc_ret = slot.derive_cap(&capability.clone().unsplay());
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

    fn set_transfer_caps_with_buf(
        &mut self,
        ep: Option<&endpoint>,
        info: &mut seL4_MessageInfo,
        current_extra_caps: &[pptr_t; seL4_MsgMaxExtraCaps],
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
        while i < seL4_MsgMaxExtraCaps && current_extra_caps[i] as usize != 0 {
            let slot = convert_to_mut_type_ref::<cte_t>(current_extra_caps[i]);
            let capability_cpy = &slot.capability.clone();
            let capability = cap::cap_endpoint_cap(capability_cpy);
            if capability.clone().unsplay().get_tag() == cap_tag::cap_endpoint_cap
                && ep.is_some()
                && capability.get_capEPPtr() as usize == ep.unwrap().get_ptr()
            {
                buffer.caps_or_badges[i] = capability.get_capEPBadge() as usize;
                info.set_capsUnwrapped(info.get_capsUnwrapped() | (1 << i));
            } else {
                if dest_slot.is_none() {
                    break;
                } else {
                    let dest = dest_slot.take();
                    let dc_ret = slot.derive_cap(&capability.clone().unsplay());
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
        let mut current_extra_caps = [0; seL4_MsgMaxExtraCaps];
        if can_grant {
            let _ = self.lookup_extra_caps(&mut current_extra_caps);
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
                    MessageID_Syscall,
                    core::cmp::min(length, n_syscallMessage),
                );
                return label as usize == 0;
            }
            seL4_Fault_tag::seL4_Fault_UserException => {
                self.copy_fault_mrs_for_reply(
                    receiver,
                    MessageID_Exception,
                    core::cmp::min(length, n_exceptionMessage),
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
            if self.do_fault_reply_transfer(receiver) {
                set_thread_state(receiver, ThreadState::ThreadStateRestart);
                possible_switch_to(receiver);
            } else {
                set_thread_state(receiver, ThreadState::ThreadStateInactive);
            }
        }
    }
}
