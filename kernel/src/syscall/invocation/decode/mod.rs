mod decode_cnode_invocation;
mod decode_domain_invocation;
pub mod decode_irq_invocation;
#[cfg(feature = "KERNEL_MCS")]
pub mod decode_sched_invocation;

pub mod arch;
mod decode_tcb_invocation;
mod decode_untyped_invocation;

use core::intrinsics::unlikely;

use log::debug;
use sel4_common::structures_gen::{cap, cap_Splayed, endpoint, notification};
use sel4_common::{
    arch::MessageLabel,
    sel4_config::seL4_InvalidCapability,
    structures::{exception_t, seL4_IPCBuffer},
    utils::convert_to_mut_type_ref,
};
use sel4_cspace::interface::cte_t;
use sel4_ipc::{endpoint_func, notification_func, Transfer};
#[cfg(not(feature = "KERNEL_MCS"))]
use sel4_task::tcb_t;
use sel4_task::{get_currenct_thread, set_thread_state, ThreadState};

use crate::kernel::boot::current_syscall_error;
use crate::syscall::invocation::decode::decode_irq_invocation::decode_irq_handler_invocation;

#[cfg(feature = "ENABLE_SMC")]
use self::arch::decode_arm_smc_invocation;
#[cfg(feature = "KERNEL_MCS")]
use self::decode_sched_invocation::{
    decode_sched_context_invocation, decode_sched_control_invocation,
};
use self::{
    arch::decode_mmu_invocation, decode_cnode_invocation::decode_cnode_invocation,
    decode_domain_invocation::decode_domain_invocation,
    decode_irq_invocation::decode_irq_control_invocation,
    decode_tcb_invocation::decode_tcb_invocation,
    decode_untyped_invocation::decode_untyed_invocation,
};
#[cfg(not(feature = "KERNEL_MCS"))]
pub fn decode_invocation(
    label: MessageLabel,
    length: usize,
    slot: &mut cte_t,
    capability: &cap,
    cap_index: usize,
    block: bool,
    call: bool,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    // sel4_common::println!("decode invocation {}", capability.get_tag());
    match capability.clone().splay() {
        cap_Splayed::null_cap(_) | cap_Splayed::zombie_cap(_) => {
            debug!(
                "Attempted to invoke a null or zombie cap {:#x}, {:?}.",
                cap_index,
                capability.get_tag()
            );
            unsafe {
                current_syscall_error._type = seL4_InvalidCapability;
                current_syscall_error.invalidCapNumber = 0;
            }
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }

        cap_Splayed::endpoint_cap(data) => {
            if unlikely(data.get_capCanSend() == 0) {
                debug!(
                    "Attempted to invoke a read-only endpoint cap {}.",
                    cap_index
                );
                unsafe {
                    current_syscall_error._type = seL4_InvalidCapability;
                    current_syscall_error.invalidCapNumber = 0;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
            set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
            convert_to_mut_type_ref::<endpoint>(data.get_capEPPtr() as usize).send_ipc(
                get_currenct_thread(),
                block,
                call,
                data.get_capCanGrant() != 0,
                data.get_capEPBadge() as usize,
                data.get_capCanGrantReply() != 0,
            );
            return exception_t::EXCEPTION_NONE;
        }

        cap_Splayed::notification_cap(data) => {
            if unlikely(data.get_capNtfnCanSend() == 0) {
                debug!(
                    "Attempted to invoke a read-only notification cap {}.",
                    cap_index
                );
                unsafe {
                    current_syscall_error._type = seL4_InvalidCapability;
                    current_syscall_error.invalidCapNumber = 0;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
            set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
            convert_to_mut_type_ref::<notification>(data.get_capNtfnPtr() as usize)
                .send_signal(data.get_capNtfnBadge() as usize);
            exception_t::EXCEPTION_NONE
        }

        cap_Splayed::reply_cap(data) => {
            if unlikely(data.get_capReplyMaster() != 0) {
                debug!("Attempted to invoke an invalid reply cap {}.", cap_index);
                unsafe {
                    current_syscall_error._type = seL4_InvalidCapability;
                    current_syscall_error.invalidCapNumber = 0;
                    return exception_t::EXCEPTION_SYSCALL_ERROR;
                }
            }
            set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
            get_currenct_thread().do_reply(
                convert_to_mut_type_ref::<tcb_t>(data.get_capTCBPtr() as usize),
                slot,
                data.get_capReplyCanGrant() != 0,
            );
            exception_t::EXCEPTION_NONE
        }
        cap_Splayed::thread_cap(data) => {
            decode_tcb_invocation(label, length, &data, slot, call, buffer)
        }
        cap_Splayed::domain_cap(_) => decode_domain_invocation(label, length, buffer),
        cap_Splayed::cnode_cap(data) => decode_cnode_invocation(label, length, &data, buffer),
        cap_Splayed::untyped_cap(data) => {
            decode_untyed_invocation(label, length, slot, &data, buffer)
        }
        cap_Splayed::irq_control_cap(_) => {
            decode_irq_control_invocation(label, length, slot, buffer)
        }
        cap_Splayed::irq_handler_cap(data) => {
            decode_irq_handler_invocation(label, data.get_capIRQ() as usize)
        }
        #[cfg(feature = "ENABLE_SMC")]
        cap_Splayed::smc_cap(data) => decode_arm_smc_invocation(label, length, &data, call, buffer),
        _ => decode_mmu_invocation(label, length, slot, call, buffer),
    }
}
#[cfg(feature = "KERNEL_MCS")]
pub fn decode_invocation(
    label: MessageLabel,
    length: usize,
    slot: &mut cte_t,
    capability: &cap,
    cap_index: usize,
    block: bool,
    call: bool,
    canDonate: bool,
    firstPhase: bool,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    // TODO: MCS , in this function, there's lot's of mcs codes
    // sel4_common::println!("decode invocation {}", capability.get_tag());

    use sel4_task::reply::reply_t;

    match capability.clone().splay() {
        cap_Splayed::null_cap(_) | cap_Splayed::zombie_cap(_) => {
            debug!(
                "Attempted to invoke a null or zombie cap {:#x}, {:?}.",
                cap_index,
                capability.get_tag()
            );
            unsafe {
                current_syscall_error._type = seL4_InvalidCapability;
                current_syscall_error.invalidCapNumber = 0;
            }
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }

        cap_Splayed::endpoint_cap(data) => {
            if unlikely(data.get_capCanSend() == 0) {
                debug!(
                    "Attempted to invoke a read-only endpoint cap {}.",
                    cap_index
                );
                unsafe {
                    current_syscall_error._type = seL4_InvalidCapability;
                    current_syscall_error.invalidCapNumber = 0;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
            set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
            convert_to_mut_type_ref::<endpoint>(data.get_capEPPtr() as usize).send_ipc(
                get_currenct_thread(),
                block,
                call,
                data.get_capCanGrant() != 0,
                data.get_capEPBadge() as usize,
                data.get_capCanGrantReply() != 0,
                canDonate,
            );
            return exception_t::EXCEPTION_NONE;
        }

        cap_Splayed::notification_cap(data) => {
            if unlikely(data.get_capNtfnCanSend() == 0) {
                debug!(
                    "Attempted to invoke a read-only notification cap {}.",
                    cap_index
                );
                unsafe {
                    current_syscall_error._type = seL4_InvalidCapability;
                    current_syscall_error.invalidCapNumber = 0;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
            set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
            convert_to_mut_type_ref::<notification>(data.get_capNtfnPtr() as usize)
                .send_signal(data.get_capNtfnBadge() as usize);
            exception_t::EXCEPTION_NONE
        }

        cap_Splayed::reply_cap(data) => {
            set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
            get_currenct_thread().do_reply(
                convert_to_mut_type_ref::<reply_t>(data.get_capReplyPtr() as usize),
                data.get_capReplyCanGrant() != 0,
            );
            exception_t::EXCEPTION_NONE
        }
        cap_Splayed::thread_cap(data) => {
            if unlikely(firstPhase) {
                debug!("Cannot invoke thread capabilities in the first phase of an invocation");
                unsafe {
                    current_syscall_error._type = seL4_InvalidCapability;
                    current_syscall_error.invalidCapNumber = 0;
                }
                return exception_t::EXCEPTION_NONE;
            }
            decode_tcb_invocation(label, length, &data, slot, call, buffer)
        }
        cap_Splayed::domain_cap(_) => {
            if unlikely(firstPhase) {
                debug!("Cannot invoke cnode capabilities in the first phase of an invocation");
                unsafe {
                    current_syscall_error._type = seL4_InvalidCapability;
                    current_syscall_error.invalidCapNumber = 0;
                }
                return exception_t::EXCEPTION_NONE;
            }
            decode_domain_invocation(label, length, buffer)
        }
        cap_Splayed::cnode_cap(data) => decode_cnode_invocation(label, length, &data, buffer),
        cap_Splayed::untyped_cap(data) => {
            decode_untyed_invocation(label, length, slot, &data, buffer)
        }
        cap_Splayed::irq_control_cap(_) => {
            decode_irq_control_invocation(label, length, slot, buffer)
        }
        cap_Splayed::irq_handler_cap(data) => {
            decode_irq_handler_invocation(label, data.get_capIRQ() as usize)
        }
        cap_Splayed::sched_control_cap(data) => {
            if unlikely(firstPhase) {
                debug!(
                    "Cannot invoke sched control capabilities in the first phase of an invocation"
                );
                unsafe {
                    current_syscall_error._type = seL4_InvalidCapability;
                    current_syscall_error.invalidCapNumber = 0;
                }
                return exception_t::EXCEPTION_NONE;
            }
            decode_sched_control_invocation(label, length, &data, buffer)
        }
        cap_Splayed::sched_context_cap(data) => {
            if unlikely(firstPhase) {
                debug!(
                    "Cannot invoke sched context capabilities in the first phase of an invocation"
                );
                unsafe {
                    current_syscall_error._type = seL4_InvalidCapability;
                    current_syscall_error.invalidCapNumber = 0;
                }
                return exception_t::EXCEPTION_NONE;
            }
            decode_sched_context_invocation(label, &data)
        }
        #[cfg(feature = "ENABLE_SMC")]
        cap_Splayed::smc_cap(data) => decode_arm_smc_invocation(label, length, &data, call, buffer),
        _ => decode_mmu_invocation(label, length, slot, call, buffer),
    }
}
