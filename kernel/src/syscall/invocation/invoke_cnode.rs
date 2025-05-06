use log::debug;
#[cfg(not(feature = "KERNEL_MCS"))]
use sel4_common::sel4_config::{seL4_DeleteFirst, tcbCaller};
use sel4_common::structures_gen::{cap, cap_tag, endpoint};
use sel4_common::{
    sel4_config::seL4_IllegalOperation, shared_types_bf_gen::seL4_CapRights,
    structures::exception_t, utils::convert_to_mut_type_ref,
};
use sel4_cspace::capability::cap_func;
use sel4_cspace::interface::{cte_insert, cte_move, cte_swap, cte_t};
use sel4_ipc::endpoint_func;
use sel4_task::{get_currenct_thread, set_thread_state, ThreadState};

use crate::{kernel::boot::current_syscall_error, syscall::mask_cap_rights};

#[inline]
pub fn invoke_cnode_copy(
    src_slot: &mut cte_t,
    dest_slot: &mut cte_t,
    cap_right: seL4_CapRights,
) -> exception_t {
    let src_cap = mask_cap_rights(cap_right, &src_slot.capability);
    let dc_ret = src_slot.derive_cap(&src_cap);
    if dc_ret.status != exception_t::EXCEPTION_NONE {
        debug!("Error deriving cap for CNode Copy operation.");
        return dc_ret.status;
    }
    if dc_ret.capability.get_tag() == cap_tag::cap_null_cap {
        debug!("CNode Copy:Copy cap would be invalid.");
        unsafe {
            current_syscall_error._type = seL4_IllegalOperation;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    cte_insert(&dc_ret.capability, src_slot, dest_slot);

    exception_t::EXCEPTION_NONE
}

#[inline]
pub fn invoke_cnode_mint(
    src_slot: &mut cte_t,
    dest_slot: &mut cte_t,
    cap_right: seL4_CapRights,
    cap_data: usize,
) -> exception_t {
    let src_cap = mask_cap_rights(cap_right, &src_slot.capability);
    let new_cap = src_cap.update_data(false, cap_data as u64);
    let dc_ret = src_slot.derive_cap(&new_cap);
    if dc_ret.status != exception_t::EXCEPTION_NONE {
        debug!("Error deriving cap for CNode Copy operation.");
        return dc_ret.status;
    }
    if dc_ret.capability.get_tag() == cap_tag::cap_null_cap {
        debug!("CNode Mint:Mint cap would be invalid.");
        unsafe {
            current_syscall_error._type = seL4_IllegalOperation;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    cte_insert(&dc_ret.capability, src_slot, dest_slot);

    exception_t::EXCEPTION_NONE
}

#[inline]
pub fn invoke_cnode_mutate(
    src_slot: &mut cte_t,
    dest_slot: &mut cte_t,
    cap_data: usize,
) -> exception_t {
    let new_cap = src_slot.capability.update_data(true, cap_data as u64);
    if new_cap.get_tag() == cap_tag::cap_null_cap {
        debug!("CNode Mint:Mint cap would be invalid.");
        unsafe {
            current_syscall_error._type = seL4_IllegalOperation;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    cte_move(&new_cap, src_slot, dest_slot);
    exception_t::EXCEPTION_NONE
}

#[inline]
#[cfg(not(feature = "KERNEL_MCS"))]
pub fn invoke_cnode_save_caller(dest_slot: &mut cte_t) -> exception_t {
    if dest_slot.capability.get_tag() != cap_tag::cap_null_cap {
        debug!("CNode SaveCaller: Destination slot not empty.");
        unsafe {
            current_syscall_error._type = seL4_DeleteFirst;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    let src_slot = get_currenct_thread().get_cspace_mut_ref(tcbCaller);
    let capability = &src_slot.clone().capability;
    match capability.get_tag() {
        cap_tag::cap_null_cap => debug!("CNode SaveCaller: Reply cap not present."),
        cap_tag::cap_reply_cap => {
            if cap::cap_reply_cap(capability).get_capReplyMaster() == 0 {
                cte_move(capability, src_slot, dest_slot);
            }
        }
        _ => panic!("caller capability must be null or reply"),
    }
    exception_t::EXCEPTION_NONE
}

#[inline]
pub fn invoke_cnode_rotate(
    slot1: &mut cte_t,
    slot2: &mut cte_t,
    slot3: &mut cte_t,
    src_new_data: usize,
    pivot_new_data: usize,
) -> exception_t {
    let new_src_cap = slot1.capability.update_data(true, src_new_data as u64);
    let new_pivot_cap = slot2.capability.update_data(true, pivot_new_data as u64);

    if new_src_cap.get_tag() == cap_tag::cap_null_cap {
        debug!("CNode Rotate: Source cap invalid");
        unsafe {
            current_syscall_error._type = seL4_IllegalOperation;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    if new_pivot_cap.get_tag() == cap_tag::cap_null_cap {
        debug!("CNode Rotate: Pivot cap invalid");
        unsafe {
            current_syscall_error._type = seL4_IllegalOperation;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);

    if slot1.get_ptr() == slot3.get_ptr() {
        cte_swap(&new_src_cap, slot1, &new_pivot_cap, slot2);
    } else {
        cte_move(&new_pivot_cap, slot2, slot3);
        cte_move(&new_src_cap, slot1, slot2);
    }

    exception_t::EXCEPTION_NONE
}

#[inline]
pub fn invoke_cnode_move(src_slot: &mut cte_t, dest_slot: &mut cte_t) -> exception_t {
    let src_cap = &src_slot.clone().capability;
    if src_cap.get_tag() == cap_tag::cap_null_cap {
        debug!("CNode Copy/Mint/Move/Mutate: Mutated cap would be invalid.");
        unsafe {
            current_syscall_error._type = seL4_IllegalOperation;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    cte_move(&src_cap, src_slot, dest_slot);
    exception_t::EXCEPTION_NONE
}

#[inline]
pub fn invoke_cnode_cancel_badged_sends(dest_slot: &mut cte_t) -> exception_t {
    let dest_cap = &dest_slot.capability;
    if !has_cancel_send_right(&dest_cap) {
        debug!("CNode CancelBadgedSends: Target cap invalid.");
        unsafe {
            current_syscall_error._type = seL4_IllegalOperation;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    let badge = cap::cap_endpoint_cap(&dest_cap).get_capEPBadge() as usize;
    if badge != 0 {
        convert_to_mut_type_ref::<endpoint>(
            cap::cap_endpoint_cap(&dest_cap).get_capEPPtr() as usize
        )
        .cancel_badged_sends(badge);
    }
    exception_t::EXCEPTION_NONE
}

#[inline]
pub fn invoke_cnode_revoke(dest_slot: &mut cte_t) -> exception_t {
    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    dest_slot.revoke()
}

#[inline]
pub fn invoke_cnode_delete(dest_slot: &mut cte_t) -> exception_t {
    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    dest_slot.delete_all(true)
}

fn has_cancel_send_right(capability: &cap) -> bool {
    match capability.get_tag() {
        cap_tag::cap_endpoint_cap => {
            cap::cap_endpoint_cap(capability).get_capCanSend() != 0
                && cap::cap_endpoint_cap(capability).get_capCanReceive() != 0
                && cap::cap_endpoint_cap(capability).get_capCanGrant() != 0
                && cap::cap_endpoint_cap(capability).get_capCanGrantReply() != 0
        }
        _ => false,
    }
}
