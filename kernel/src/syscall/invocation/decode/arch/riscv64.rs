use core::intrinsics::unlikely;

// use crate::{common::{
//     message_info::MessageLabel, structures::{exception_t, seL4_IPCBuffer},
//     sel4_config::*, utils::{convert_to_mut_type_ref, pageBitsForSize}, fault::*,
// }, BIT, MASK};

use log::debug;
use sel4_common::arch::MessageLabel;
use sel4_common::sel4_config::{
    asidInvalid, asidLowBits, nASIDPools, seL4_AlignmentError, seL4_DeleteFirst, seL4_FailedLookup,
    seL4_IllegalOperation, seL4_InvalidArgument, seL4_InvalidCapability, seL4_PageBits,
    seL4_RevokeFirst, seL4_TruncatedMessage,
};
use sel4_common::structures::{exception_t, seL4_IPCBuffer};
use sel4_common::structures_gen::{cap, cap_page_table_cap, cap_tag};
use sel4_common::structures_gen::{lookup_fault_invalid_root, lookup_fault_missing_capability};
use sel4_common::utils::{convert_to_mut_type_ref, pageBitsForSize};
use sel4_common::{BIT, MASK};
use sel4_cspace::interface::cte_t;
use sel4_task::{get_currenct_thread, set_thread_state, ThreadState};
use sel4_vspace::{
    checkVPAlignment, find_vspace_for_asid, get_asid_pool_by_index, vm_attributes_t, PTE,
};

use crate::{
    config::{seL4_ASIDPoolBits, USER_TOP},
    kernel::boot::{current_lookup_fault, current_syscall_error, get_extra_cap_by_index},
    syscall::{
        get_syscall_arg,
        invocation::invoke_mmu_op::{
            invoke_asid_control, invoke_asid_pool, invoke_page_get_address, invoke_page_map,
            invoke_page_table_map, invoke_page_table_unmap, invoke_page_unmap,
        },
        lookup_slot_for_cnode_op,
    },
};

use sel4_common::sel4_config::seL4_RangeError;

use crate::{
    config::{irqInvalid, maxIRQ},
    interrupt::is_irq_active,
    syscall::{invocation::invoke_irq::invoke_irq_control, lookupSlotForCNodeOp},
};

pub fn decode_mmu_invocation(
    label: MessageLabel,
    length: usize,
    slot: &mut cte_t,
    call: bool,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    match slot.capability.get_tag() {
        cap_tag::cap_page_table_cap => decode_page_table_invocation(label, length, slot, buffer),
        cap_tag::cap_frame_cap => decode_frame_invocation(label, length, slot, call, buffer),
        cap_tag::cap_asid_control_cap => decode_asid_control(label, length, buffer),
        cap_tag::cap_asid_pool_cap => decode_asid_pool(label, slot),
        _ => {
            panic!("Invalid arch cap type");
        }
    }
}

fn decode_page_table_invocation(
    label: MessageLabel,
    length: usize,
    cte: &mut cte_t,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    match label {
        MessageLabel::RISCVPageTableUnmap => decode_page_table_unmap(cte),

        MessageLabel::RISCVPageTableMap => decode_page_table_map(length, cte, buffer),
        _ => {
            debug!("RISCVPageTable: Illegal Operation");
            unsafe {
                current_syscall_error._type = seL4_IllegalOperation;
            }
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }
    }
}

fn decode_frame_invocation(
    label: MessageLabel,
    length: usize,
    frame_slot: &mut cte_t,
    call: bool,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    match label {
        MessageLabel::RISCVPageMap => decode_frame_map(length, frame_slot, buffer),
        MessageLabel::RISCVPageUnmap => {
            set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
            invoke_page_unmap(frame_slot)
        }
        MessageLabel::RISCVPageGetAddress => {
            set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
            invoke_page_get_address(
                cap::cap_frame_cap(&frame_slot.capability).get_capFBasePtr() as usize,
                call,
            )
        }
        _ => {
            debug!("invalid operation label:{:?}", label);
            unsafe {
                current_syscall_error._type = seL4_IllegalOperation;
            }
            exception_t::EXCEPTION_SYSCALL_ERROR
        }
    }
}

fn decode_asid_control(label: MessageLabel, length: usize, buffer: &seL4_IPCBuffer) -> exception_t {
    if label != MessageLabel::RISCVASIDControlMakePool {
        unsafe {
            current_syscall_error._type = seL4_IllegalOperation;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    if unlikely(
        length < 2 || get_extra_cap_by_index(0).is_none() || get_extra_cap_by_index(1).is_none(),
    ) {
        unsafe {
            current_syscall_error._type = seL4_TruncatedMessage;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    let index = get_syscall_arg(0, buffer);
    let depth = get_syscall_arg(1, buffer);
    let parent_slot = get_extra_cap_by_index(0).unwrap();
    let untyped_cap = &parent_slot.capability;
    let root = &get_extra_cap_by_index(1).unwrap().capability;

    let mut i = 0;
    while get_asid_pool_by_index(i).is_some() {
        i += 1;
    }

    if i == nASIDPools {
        unsafe {
            current_syscall_error._type = seL4_DeleteFirst;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let asid_base = i << asidLowBits;
    if untyped_cap.get_tag() != cap_tag::cap_untyped_cap
        || cap::cap_untyped_cap(&untyped_cap).get_capBlockSize() as usize != seL4_ASIDPoolBits
        || cap::cap_untyped_cap(&untyped_cap).get_capIsDevice() != 0
    {
        unsafe {
            current_syscall_error._type = seL4_InvalidCapability;
            current_syscall_error.invalidCapNumber = 1;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let status = parent_slot.ensure_no_children();
    if status != exception_t::EXCEPTION_NONE {
        unsafe {
            current_syscall_error._type = seL4_RevokeFirst;
        }
        return status;
    }

    let frame = cap::cap_untyped_cap(&untyped_cap).get_capPtr();
    let lu_ret = lookup_slot_for_cnode_op(false, root, index, depth);
    if lu_ret.status != exception_t::EXCEPTION_NONE {
        return lu_ret.status;
    }

    let dest_slot = convert_to_mut_type_ref::<cte_t>(lu_ret.slot as usize);

    if dest_slot.capability.get_tag() != cap_tag::cap_null_cap {
        unsafe {
            current_syscall_error._type = seL4_DeleteFirst;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    invoke_asid_control(frame as usize, dest_slot, parent_slot, asid_base)
}

fn decode_asid_pool(label: MessageLabel, cte: &mut cte_t) -> exception_t {
    // debug!("in cap_asid_pool_cap");
    if label != MessageLabel::RISCVASIDPoolAssign {
        unsafe {
            current_syscall_error._type = seL4_IllegalOperation;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    if unlikely(get_extra_cap_by_index(0).is_none()) {
        unsafe {
            current_syscall_error._type = seL4_TruncatedMessage;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let vspace_slot = get_extra_cap_by_index(0).unwrap();
    let vspace_cap = &vspace_slot.capability;

    if unlikely(
        vspace_cap.get_tag() != cap_tag::cap_page_table_cap
            || cap::cap_page_table_cap(&vspace_cap).get_capPTIsMapped() != 0,
    ) {
        debug!("RISCVASIDPool: Invalid vspace root.");
        unsafe {
            current_syscall_error._type = seL4_InvalidCapability;
            current_syscall_error.invalidCapNumber = 1;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let asid = cap::cap_asid_pool_cap(&cte.capability).get_capASIDBase() as usize;
    if let Some(pool) = get_asid_pool_by_index(asid >> asidLowBits) {
        if pool.get_ptr() != cap::cap_asid_pool_cap(&cte.capability).get_capASIDPool() as usize {
            unsafe {
                current_syscall_error._type = seL4_InvalidCapability;
                current_syscall_error.invalidCapNumber = 0;
            }
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }

        let mut i = 0;
        while i < BIT!(asidLowBits) && (asid + i == 0 || pool.get_vspace_by_index(i).is_some()) {
            i += 1;
        }

        if i == BIT!(asidLowBits) {
            unsafe {
                current_syscall_error._type = seL4_DeleteFirst;
            }
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }

        set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
        // performASIDPoolInvocation(asid + i, pool as *mut asid_pool_t, vspace_slot as *mut cte_t)
        invoke_asid_pool(asid + i, pool, vspace_slot)
    } else {
        unsafe {
            current_syscall_error._type = seL4_FailedLookup;
            current_syscall_error.failedLookupWasSource = 0;
            current_lookup_fault = lookup_fault_invalid_root::new().unsplay();
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
}

fn decode_frame_map(length: usize, frame_slot: &mut cte_t, buffer: &seL4_IPCBuffer) -> exception_t {
    if length < 3 || get_extra_cap_by_index(0).is_none() {
        debug!("RISCVPageMap: Truncated message.");
        unsafe {
            current_syscall_error._type = seL4_TruncatedMessage;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let vaddr = get_syscall_arg(0, buffer);
    let w_rights_mask = get_syscall_arg(1, buffer);
    let attr = vm_attributes_t::from_word(get_syscall_arg(2, buffer));
    let lvl1pt_cap = cap::cap_page_table_cap(&get_extra_cap_by_index(0).unwrap().capability);
    if let Some((lvl1pt, asid)) = get_vspace(&lvl1pt_cap) {
        let frame_size = cap::cap_frame_cap(&frame_slot.capability).get_capFSize() as usize;
        let vtop = vaddr + BIT!(pageBitsForSize(frame_size)) - 1;
        if unlikely(vtop >= USER_TOP) {
            unsafe {
                current_syscall_error._type = seL4_InvalidArgument;
                current_syscall_error.invalidCapNumber = 0;
            }
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }

        if unlikely(!checkVPAlignment(frame_size, vaddr)) {
            unsafe {
                current_syscall_error._type = seL4_AlignmentError;
            }
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }

        let lu_ret = lvl1pt.lookup_pt_slot(vaddr);
        if lu_ret.ptBitsLeft != pageBitsForSize(frame_size) {
            unsafe {
                current_lookup_fault =
                    lookup_fault_missing_capability::new(lu_ret.ptBitsLeft as u64).unsplay();
                current_syscall_error._type = seL4_FailedLookup;
                current_syscall_error.failedLookupWasSource = false as usize;
            }
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }

        let pt_slot = convert_to_mut_type_ref::<PTE>(lu_ret.ptSlot as usize);
        let frame_asid = cap::cap_frame_cap(&frame_slot.capability).get_capFMappedASID() as usize;
        if frame_asid != asidInvalid {
            if frame_asid != asid {
                debug!("RISCVPageMap: Attempting to remap a frame that does not belong to the passed address space");
                unsafe {
                    current_syscall_error._type = seL4_InvalidCapability;
                    current_syscall_error.invalidCapNumber = 1;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }

            if cap::cap_frame_cap(&frame_slot.capability).get_capFMappedAddress() as usize != vaddr
            {
                debug!("RISCVPageMap: attempting to map frame into multiple addresses");
                unsafe {
                    current_syscall_error._type = seL4_InvalidArgument;
                    current_syscall_error.invalidArgumentNumber = 0;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }

            if pt_slot.is_pte_table() {
                debug!("RISCVPageMap: no mapping to remap.");
                unsafe {
                    current_syscall_error._type = seL4_DeleteFirst;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
        } else {
            if pt_slot.get_valid() != 0 {
                debug!("Virtual address already mapped");
                unsafe {
                    current_syscall_error._type = seL4_DeleteFirst;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
        }
        invoke_page_map(
            cap::cap_frame_cap(&frame_slot.clone().capability),
            w_rights_mask,
            vaddr,
            asid,
            attr,
            pt_slot,
            frame_slot,
        )
    } else {
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
}

fn decode_page_table_unmap(pt_cte: &mut cte_t) -> exception_t {
    if !pt_cte.is_final_cap() {
        debug!("RISCVPageTableUnmap: cannot unmap if more than once cap exists");
        unsafe {
            current_syscall_error._type = seL4_RevokeFirst;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    let capability = cap::cap_page_table_cap(&pt_cte.capability);
    if capability.get_capPTIsMapped() != 0 {
        let asid = capability.get_capPTMappedASID() as usize;
        let find_ret = find_vspace_for_asid(asid);
        let pte_ptr = capability.get_capPTBasePtr() as *mut PTE;
        if find_ret.status == exception_t::EXCEPTION_NONE
            && find_ret.vspace_root.unwrap() == pte_ptr
        {
            debug!("RISCVPageTableUnmap: cannot call unmap on top level PageTable");
            unsafe {
                current_syscall_error._type = seL4_RevokeFirst;
            }
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        } else {
            unsafe {
                current_lookup_fault = find_ret.lookup_fault.unwrap();
            }
        }
    }
    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    return invoke_page_table_unmap(capability);
}

fn decode_page_table_map(
    length: usize,
    pt_cte: &mut cte_t,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    if unlikely(length < 2 || get_extra_cap_by_index(0).is_none()) {
        debug!("RISCVPageTableMap: truncated message");
        unsafe {
            current_syscall_error._type = seL4_TruncatedMessage;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    let capability = cap::cap_page_table_cap(&pt_cte.capability);
    if unlikely(capability.get_capPTIsMapped() != 0) {
        debug!("RISCVPageTable: PageTable is already mapped.");
        unsafe {
            current_syscall_error._type = seL4_InvalidCapability;
            current_syscall_error.invalidCapNumber = 0;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let vaddr = get_syscall_arg(0, buffer);
    if unlikely(vaddr >= USER_TOP) {
        debug!("RISCVPageTableMap: Virtual address cannot be in kernel window.");
        unsafe {
            current_syscall_error._type = seL4_InvalidArgument;
            current_syscall_error.invalidCapNumber = 0;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    let lvl1pt_cap = cap::cap_page_table_cap(&get_extra_cap_by_index(0).unwrap().capability);

    if let Some((lvl1pt, asid)) = get_vspace(&lvl1pt_cap) {
        let lu_ret = lvl1pt.lookup_pt_slot(vaddr);
        let lu_slot = convert_to_mut_type_ref::<PTE>(lu_ret.ptSlot as usize);
        #[cfg(target_arch = "riscv64")]
        if lu_ret.ptBitsLeft == seL4_PageBits || lu_slot.get_valid() != 0 {
            debug!("RISCVPageTableMap: All objects mapped at this address");
            unsafe {
                current_syscall_error._type = seL4_DeleteFirst;
            }
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }
        set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
        return invoke_page_table_map(capability, lu_slot, asid, vaddr & !MASK!(lu_ret.ptBitsLeft));
    } else {
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
}

fn get_vspace(lvl1pt_cap: &cap_page_table_cap) -> Option<(&mut PTE, usize)> {
    if lvl1pt_cap.clone().unsplay().get_tag() != cap_tag::cap_page_table_cap
        || lvl1pt_cap.get_capPTIsMapped() as usize == asidInvalid
    {
        debug!("RISCVMMUInvocation: Invalid top-level PageTable.");
        unsafe {
            current_syscall_error._type = seL4_InvalidCapability;
            current_syscall_error.invalidCapNumber = 1;
        }
        return None;
    }

    let lvl1pt = convert_to_mut_type_ref::<PTE>(lvl1pt_cap.get_capPTBasePtr() as usize);
    let asid = lvl1pt_cap.get_capPTMappedASID() as usize;

    let find_ret = find_vspace_for_asid(asid);
    if find_ret.status != exception_t::EXCEPTION_NONE {
        debug!("RISCVMMUInvocation: ASID lookup failed");
        unsafe {
            current_lookup_fault = find_ret.lookup_fault.unwrap();
            current_syscall_error._type = seL4_FailedLookup;
            current_syscall_error.failedLookupWasSource = 0;
        }
        return None;
    }

    if find_ret.vspace_root.unwrap() as usize != lvl1pt.get_ptr() {
        debug!("RISCVMMUInvocation: ASID lookup failed");
        unsafe {
            current_syscall_error._type = seL4_InvalidCapability;
            current_syscall_error.invalidCapNumber = 1;
        }
        return None;
    }
    Some((lvl1pt, asid))
}

pub(crate) fn check_irq(irq: usize) -> exception_t {
    if irq > maxIRQ || irq == irqInvalid {
        unsafe {
            current_syscall_error._type = seL4_RangeError;
            current_syscall_error.rangeErrorMin = 1;
            current_syscall_error.rangeErrorMax = maxIRQ;
            debug!(
                "Rejecting request for IRQ {}. IRQ is out of range [1..maxIRQ].",
                irq
            );
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }
    }
    exception_t::EXCEPTION_NONE
}

pub fn arch_decode_irq_control_invocation(
    label: MessageLabel,
    length: usize,
    src_slot: &mut cte_t,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    if label == MessageLabel::RISCVIRQIssueIRQHandlerTrigger {
        if length < 4 || get_extra_cap_by_index(0).is_none() {
            unsafe {
                current_syscall_error._type = seL4_TruncatedMessage;
            }
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }
        let irq = get_syscall_arg(0, buffer);
        let _trigger = get_syscall_arg(1, buffer) != 0;
        let index = get_syscall_arg(2, buffer);
        let depth = get_syscall_arg(3, buffer);
        let cnode_cap = &get_extra_cap_by_index(0).unwrap().capability;
        let status = check_irq(irq);
        if status != exception_t::EXCEPTION_NONE {
            return status;
        }
        if is_irq_active(irq) {
            unsafe {
                current_syscall_error._type = seL4_RevokeFirst;
            }
            debug!("Rejecting request for IRQ {}. Already active.", irq);
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }
        let lu_ret = lookupSlotForCNodeOp(false, cnode_cap, index, depth);
        if lu_ret.status != exception_t::EXCEPTION_NONE {
            debug!("Target slot for new IRQ Handler cap invalid: IRQ {}.", irq);
            return lu_ret.status;
        }
        set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
        invoke_irq_control(
            irq,
            convert_to_mut_type_ref::<cte_t>(lu_ret.slot as usize),
            src_slot,
        )
    } else {
        unsafe {
            current_syscall_error._type = seL4_IllegalOperation;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
}
