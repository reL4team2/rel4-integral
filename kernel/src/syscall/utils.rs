use core::intrinsics::unlikely;

use crate::kernel::boot::current_extra_caps;
use crate::{
    kernel::boot::{current_lookup_fault, current_syscall_error},
    BIT, IS_ALIGNED, MASK,
};
use log::debug;
use sel4_common::arch::{maskVMRights, ArchReg, MSG_REGISTER_NUM};
use sel4_common::ffi::current_fault;
use sel4_common::sel4_config::SEL4_MIN_UNTYPED_BITS;
use sel4_common::shared_types_bf_gen::seL4_CapRights;
use sel4_common::structures_gen::{
    cap, cap_Splayed, cap_tag, lookup_fault_depth_mismatch, lookup_fault_invalid_root, notification,
};
use sel4_common::{
    sel4_config::*,
    structures::{exception_t, seL4_IPCBuffer},
};
use sel4_common::{
    sel4_config::{
        SEL4_ALIGNMENT_ERROR, SEL4_DELETE_FIRST, SEL4_FAILED_LOOKUP, SEL4_IPC_BUFFER_SIZE_BITS,
        WORD_BITS,
    },
    utils::convert_to_mut_type_ref,
};
use sel4_cspace::arch::arch_mask_cap_rights;
use sel4_cspace::capability::cap_func;
use sel4_cspace::interface::{cte_t, resolve_address_bits};
use sel4_ipc::notification_func;
use sel4_task::{get_currenct_thread, lookupSlot_ret_t, tcb_t};

pub fn alignUp(baseValue: usize, alignment: usize) -> usize {
    (baseValue + BIT!(alignment) - 1) & !MASK!(alignment)
}

pub fn FREE_INDEX_TO_OFFSET(freeIndex: usize) -> usize {
    freeIndex << SEL4_MIN_UNTYPED_BITS
}
pub fn GET_FREE_REF(base: usize, freeIndex: usize) -> usize {
    base + FREE_INDEX_TO_OFFSET(freeIndex)
}
pub fn GET_FREE_INDEX(base: usize, free: usize) -> usize {
    free - base >> SEL4_MIN_UNTYPED_BITS
}
pub fn GET_OFFSET_FREE_PTR(base: usize, offset: usize) -> *mut usize {
    (base + offset) as *mut usize
}
pub fn OFFSET_TO_FREE_IDNEX(offset: usize) -> usize {
    offset >> SEL4_MIN_UNTYPED_BITS
}

// #[inline]
// #[no_mangle]
// pub fn getSyscallArg(i: usize, ipc_buffer: *const usize) -> usize {
//     unsafe {
//         return if i < MSG_REGISTER_NUM {
//             // return getRegister(get_currenct_thread() as *const tcb_t, MSG_REGISTER[i]);
//             get_currenct_thread().tcbArch.get_register(ArchReg::Msg(i))
//         } else {
//             assert_ne!(ipc_buffer as usize, 0);
//             let ptr = ipc_buffer.add(i + 1);
//             *ptr
//         };
//     }
// }

#[inline]
pub fn lookup_extra_caps_with_buf(thread: &mut tcb_t, buf: Option<&seL4_IPCBuffer>) -> exception_t {
    unsafe {
        match thread.lookup_extra_caps_with_buf(&mut current_extra_caps.excaprefs, buf) {
            Ok(()) => {}
            Err(fault) => {
                current_fault = fault;
                return exception_t::EXCEPTION_LOOKUP_FAULT;
            }
        }
    }
    return exception_t::EXCEPTION_NONE;
}

// TODO: Remove this option because it not need to check whether is None or Some
#[inline]
pub fn get_syscall_arg(i: usize, ipc_buffer: &seL4_IPCBuffer) -> usize {
    match i < MSG_REGISTER_NUM {
        true => get_currenct_thread().tcbArch.get_register(ArchReg::Msg(i)),
        false => ipc_buffer.msg[i],
    }
}

#[inline]
pub fn check_prio(prio: usize, auth_tcb: &tcb_t) -> exception_t {
    if prio > auth_tcb.tcbMCP {
        unsafe {
            current_syscall_error._type = SEL4_RANGE_ERROR;
            current_syscall_error.rangeErrorMin = SEL4_MIN_PRIO;
            current_syscall_error.rangeErrorMax = auth_tcb.tcbMCP;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    exception_t::EXCEPTION_NONE
}

#[inline]
pub fn check_ipc_buffer_vaild(vptr: usize, capability: &cap) -> exception_t {
    if capability.clone().get_tag() != cap_tag::cap_frame_cap {
        debug!("Requested IPC Buffer is not a frame cap.");
        unsafe {
            current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    if cap::cap_frame_cap(capability).get_capFIsDevice() != 0 {
        debug!("Specifying a device frame as an IPC buffer is not permitted.");
        unsafe {
            current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    if !IS_ALIGNED!(vptr, SEL4_IPC_BUFFER_SIZE_BITS) {
        debug!("Requested IPC Buffer location 0x%x is not aligned.");
        unsafe {
            current_syscall_error._type = SEL4_ALIGNMENT_ERROR;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    exception_t::EXCEPTION_NONE
}

#[inline]
pub fn do_bind_notification(tcb: &mut tcb_t, nftn: &mut notification) {
    nftn.bind_tcb(tcb);
    tcb.bind_notification(nftn.get_ptr());
}

#[inline]
pub fn do_unbind_notification(tcb: &mut tcb_t, nftn: &mut notification) {
    nftn.unbind_tcb();
    tcb.unbind_notification();
}

#[inline]
pub fn safe_unbind_notification(tcb: &mut tcb_t) {
    let nftn = tcb.tcbBoundNotification;
    if nftn != 0 {
        do_unbind_notification(tcb, convert_to_mut_type_ref::<notification>(nftn))
    }
}

#[inline]
#[cfg(target_arch = "riscv64")]
pub fn is_valid_vtable_root(capability: &cap) -> bool {
    capability.get_tag() == cap_tag::cap_page_table_cap
        && cap::cap_page_table_cap(capability).get_capPTIsMapped() != 0
}

#[no_mangle]
pub fn isValidVTableRoot(_cap: &cap) -> bool {
    panic!("should not be invoked!")
}

pub fn lookup_slot_for_cnode_op(
    is_source: bool,
    root: &cap,
    cap_ptr: usize,
    depth: usize,
) -> lookupSlot_ret_t {
    let mut ret: lookupSlot_ret_t = lookupSlot_ret_t::default();
    if unlikely(root.clone().get_tag() != cap_tag::cap_cnode_cap) {
        unsafe {
            current_syscall_error._type = SEL4_FAILED_LOOKUP;
            current_syscall_error.failedLookupWasSource = is_source as usize;
            current_lookup_fault = lookup_fault_invalid_root::new().unsplay();
        }
        ret.status = exception_t::EXCEPTION_SYSCALL_ERROR;
        return ret;
    }

    if unlikely(depth < 1 || depth > WORD_BITS) {
        unsafe {
            current_syscall_error._type = SEL4_RANGE_ERROR;
            current_syscall_error.rangeErrorMin = 1;
            current_syscall_error.rangeErrorMax = WORD_BITS;
        }
        ret.status = exception_t::EXCEPTION_SYSCALL_ERROR;
        return ret;
    }
    let res_ret = resolve_address_bits(root, cap_ptr, depth);
    if unlikely(res_ret.status != exception_t::EXCEPTION_NONE) {
        unsafe {
            current_syscall_error._type = SEL4_FAILED_LOOKUP;
            current_syscall_error.failedLookupWasSource = is_source as usize;
        }
        ret.status = exception_t::EXCEPTION_SYSCALL_ERROR;
        return ret;
    }

    if unlikely(res_ret.bitsRemaining != 0) {
        unsafe {
            current_syscall_error._type = SEL4_FAILED_LOOKUP;
            current_syscall_error.failedLookupWasSource = is_source as usize;
            current_lookup_fault =
                lookup_fault_depth_mismatch::new(0, res_ret.bitsRemaining as u64).unsplay();
        }
        ret.status = exception_t::EXCEPTION_SYSCALL_ERROR;
        return ret;
    }
    ret.slot = res_ret.slot;
    ret.status = exception_t::EXCEPTION_NONE;
    ret
}

pub fn lookupSlotForCNodeOp(
    isSource: bool,
    root: &cap,
    capptr: usize,
    depth: usize,
) -> lookupSlot_ret_t {
    lookup_slot_for_cnode_op(isSource, root, capptr, depth)
}

#[inline]
pub fn ensure_empty_slot(slot: &cte_t) -> exception_t {
    if slot.capability.get_tag() != cap_tag::cap_null_cap {
        unsafe {
            current_syscall_error._type = SEL4_DELETE_FIRST;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    exception_t::EXCEPTION_NONE
}

#[no_mangle]
pub fn ensureEmptySlot(slot: *mut cte_t) -> exception_t {
    unsafe { ensure_empty_slot(&*slot) }
}

pub fn mask_cap_rights(rights: seL4_CapRights, capability: &cap) -> cap {
    if capability.is_arch_cap() {
        return arch_mask_cap_rights(rights, capability);
    }
    match capability.clone().splay() {
        cap_Splayed::endpoint_cap(data) => {
            let capability_copy = &capability.clone();
            let new_cap = cap::cap_endpoint_cap(capability_copy);
            new_cap.set_capCanSend(data.get_capCanSend() & rights.get_capAllowWrite() as u64);
            new_cap.set_capCanReceive(data.get_capCanReceive() & rights.get_capAllowRead() as u64);
            new_cap.set_capCanGrant(data.get_capCanGrant() & rights.get_capAllowGrant() as u64);
            new_cap.set_capCanGrantReply(
                data.get_capCanGrantReply() & rights.get_capAllowGrantReply() as u64,
            );
            capability_copy.clone()
        }
        cap_Splayed::notification_cap(data) => {
            let capability_copy = &capability.clone();
            let new_cap = cap::cap_notification_cap(capability_copy);
            new_cap
                .set_capNtfnCanSend(data.get_capNtfnCanSend() & rights.get_capAllowWrite() as u64);
            new_cap.set_capNtfnCanReceive(
                data.get_capNtfnCanReceive() & rights.get_capAllowRead() as u64,
            );
            capability_copy.clone()
        }
        cap_Splayed::reply_cap(data) => {
            let capability_copy = &capability.clone();
            let new_cap = cap::cap_reply_cap(capability_copy);
            new_cap.set_capReplyCanGrant(
                data.get_capReplyCanGrant() & rights.get_capAllowGrant() as u64,
            );
            capability_copy.clone()
        }
        cap_Splayed::frame_cap(data) => {
            let capability_copy = &capability.clone();
            let new_cap = cap::cap_frame_cap(capability_copy);
            let mut vm_rights = unsafe { core::mem::transmute(data.get_capFVMRights()) };
            vm_rights = maskVMRights(vm_rights, rights);
            new_cap.set_capFVMRights(vm_rights as u64);
            capability_copy.clone()
        }
        _ => capability.clone(),
    }
}
