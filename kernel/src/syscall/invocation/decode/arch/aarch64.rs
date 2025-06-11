use crate::arch::set_vm_root_for_flush;
use crate::kernel::boot::{current_extra_caps, get_extra_cap_by_index};
use crate::syscall::invocation::decode::current_syscall_error;
use crate::syscall::ThreadState;
use crate::syscall::{current_lookup_fault, get_syscall_arg, set_thread_state, unlikely};
use crate::syscall::{ensure_empty_slot, get_currenct_thread, lookup_slot_for_cnode_op};
use log::debug;
use sel4_common::arch::maskVMRights;
use sel4_common::platform::MAX_IRQ;
use sel4_common::sel4_bitfield_types::Bitfield;
use sel4_common::sel4_config::{
    ASID_INVALID, ASID_LOW_BITS, N_ASID_POOLS, SEL4_ALIGNMENT_ERROR, SEL4_ASID_POOL_BITS,
    SEL4_FAILED_LOOKUP, SEL4_PAGE_BITS, SEL4_RANGE_ERROR, USER_TOP,
};
use sel4_common::sel4_config::{SEL4_DELETE_FIRST, SEL4_INVALID_ARGUMENT};
use sel4_common::sel4_config::{
    SEL4_ILLEGAL_OPERATION, SEL4_INVALID_CAPABILITY, SEL4_REVOKE_FIRST, SEL4_TRUNCATED_MESSAGE,
};
use sel4_common::shared_types_bf_gen::seL4_CapRights;
use sel4_common::structures_gen::{
    asid_map_asid_map_vspace, cap, cap_Splayed, cap_asid_pool_cap, cap_tag, cap_vspace_cap,
};
use sel4_common::structures_gen::{lookup_fault_invalid_root, lookup_fault_missing_capability};
use sel4_common::utils::{
    convert_ref_type_to_usize, convert_to_mut_type_ref, global_ops, max_free_index,
    pageBitsForSize, ptr_to_mut, ptr_to_ref,
};
use sel4_common::{
    arch::MessageLabel,
    structures::{exception_t, seL4_IPCBuffer},
    MASK,
};
use sel4_common::{BIT, IS_ALIGNED};
use sel4_cspace::capability::cap_arch_func;
use sel4_cspace::interface::{cte_insert, cte_t};

use sel4_vspace::{
    asid_pool_t, asid_t, clean_by_va_pou, do_flush, find_vspace_for_asid, get_asid_pool_by_index,
    pptr_to_paddr, pte_tag_t, set_asid_pool_by_index, vm_attributes_t, vptr_t, PTE,
};

use crate::syscall::invocation::invoke_mmu_op::{
    invoke_page_get_address, invoke_page_map, invoke_page_table_unmap, invoke_page_unmap,
};
use crate::{
    interrupt::is_irq_active,
    syscall::{invocation::invoke_irq::invoke_irq_control, lookupSlotForCNodeOp},
};
#[cfg(feature = "enable_smp")]
use sel4_common::platform::NUM_PPI;
#[cfg(feature = "enable_smp")]
use sel4_common::sel4_config::CONFIG_MAX_NUM_NODES;
#[cfg(feature = "enable_smc")]
use sel4_common::sel4_config::NUM_SMC_REGS;
#[cfg(feature = "enable_smp")]
use sel4_common::structures::{irq_to_idx, irqt_to_irq, to_irqt};
#[cfg(feature = "enable_smp")]
use crate::arch::arm_gic::gic_v2::gic_v2::set_irq_target;
#[cfg(feature = "enable_smc")]
use sel4_common::{
    arch::ArchReg, arch::MessageLabel::ARMSMCCall, arch::MSG_REGISTER_NUM,
    structures_gen::cap_smc_cap,
};

pub fn decode_mmu_invocation(
    label: MessageLabel,
    length: usize,
    slot: &mut cte_t,
    call: bool,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    match slot.capability.clone().splay() {
        cap_Splayed::vspace_cap(_) => decode_vspace_root_invocation(label, length, slot, buffer),
        cap_Splayed::page_table_cap(_) => decode_page_table_invocation(label, length, slot, buffer),
        cap_Splayed::frame_cap(_) => decode_frame_invocation(label, length, slot, call, buffer),
        cap_Splayed::asid_control_cap(_) => decode_asid_control(label, length, buffer),
        cap_Splayed::asid_pool_cap(_) => decode_asid_pool(label, slot),
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
    /*
        if (invLabel == ARMPageTableUnmap) {
            if (unlikely(!isFinalCapability(cte))) {
                current_syscall_error.type = SEL4_REVOKE_FIRST;
                return EXCEPTION_SYSCALL_ERROR;
            }
            set_thread_state(NODE_STATE(ksCurThread), ThreadState_Restart);
            return performPageTableInvocationUnmap(cap, cte);
        }
    */

    if label == MessageLabel::ARMPageTableUnmap {
        if unlikely(!cte.is_final_cap()) {
            global_ops!(current_syscall_error._type = SEL4_REVOKE_FIRST);
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }
        // log::warn!("Need to check is FinalCapability here");
        get_currenct_thread().set_state(ThreadState::ThreadStateRestart);
        // unimplemented!("performPageTableInvocationUnmap");
        return decode_page_table_unmap(cte);
    }

    if unlikely(label != MessageLabel::ARMPageTableMap) {
        global_ops!(current_syscall_error._type = SEL4_ILLEGAL_OPERATION);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    if unlikely(length < 2 || global_ops!(current_extra_caps.excaprefs[0] == 0)) {
        global_ops!(current_syscall_error._type = SEL4_TRUNCATED_MESSAGE);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    if unlikely(cap::cap_page_table_cap(&cte.capability).get_capPTIsMapped() == 1) {
        global_ops!(current_syscall_error._type = SEL4_INVALID_CAPABILITY);
        global_ops!(current_syscall_error.invalidArgumentNumber = 0);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let vaddr = get_syscall_arg(0, buffer);
    let vspace_root_cap =
        convert_to_mut_type_ref::<cap_vspace_cap>(global_ops!(current_extra_caps.excaprefs[0]));

    if unlikely(!vspace_root_cap.clone().unsplay().is_valid_native_root()) {
        global_ops!(current_syscall_error._type = SEL4_INVALID_CAPABILITY);
        global_ops!(current_syscall_error.invalidCapNumber = 1);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let vspace_root = vspace_root_cap.get_capVSBasePtr() as usize;
    let asid = vspace_root_cap.get_capVSMappedASID() as usize;

    if unlikely(vaddr > USER_TOP) {
        global_ops!(current_syscall_error._type = SEL4_INVALID_ARGUMENT);
        global_ops!(current_syscall_error.invalidArgumentNumber = 0);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let find_ret = find_vspace_for_asid(asid);

    if unlikely(find_ret.status != exception_t::EXCEPTION_NONE) {
        global_ops!(current_syscall_error._type = SEL4_FAILED_LOOKUP);
        global_ops!(current_syscall_error.failedLookupWasSource = 0);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    if unlikely(find_ret.vspace_root.unwrap() as usize != vspace_root) {
        global_ops!(current_syscall_error._type = SEL4_INVALID_CAPABILITY);
        global_ops!(current_syscall_error.invalidCapNumber = 1);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let pd_slot = PTE(vspace_root).lookup_pt_slot(vaddr);

    if unlikely(
        pd_slot.ptBitsLeft == SEL4_PAGE_BITS
            || (ptr_to_ref(pd_slot.ptSlot).get_type() != (pte_tag_t::pte_invalid) as usize),
    ) {
        global_ops!(current_syscall_error._type = SEL4_DELETE_FIRST);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    let pte = PTE::pte_new_table(pptr_to_paddr(
        cap::cap_page_table_cap(&cte.capability).get_capPTBasePtr() as usize,
    ));
    cap::cap_page_table_cap(&cte.capability).set_capPTIsMapped(1);
    cap::cap_page_table_cap(&cte.capability).set_capPTMappedASID(asid as u64);
    cap::cap_page_table_cap(&cte.capability)
        .set_capPTMappedAddress((vaddr & !(MASK!(pd_slot.ptBitsLeft))) as u64);
    get_currenct_thread().set_state(ThreadState::ThreadStateRestart);

    *ptr_to_mut(pd_slot.ptSlot) = pte;
    // log::warn!("Need to clean D-Cache using cleanByVA_PoU");
    clean_by_va_pou(
        convert_ref_type_to_usize(ptr_to_mut(pd_slot.ptSlot)),
        pptr_to_paddr(convert_ref_type_to_usize(ptr_to_mut(pd_slot.ptSlot))),
    );
    exception_t::EXCEPTION_NONE
}

fn decode_page_clean_invocation(
    label: MessageLabel,
    length: usize,
    cte: &mut cte_t,
    _call: bool,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    if length < 2 {
        log::error!("[User] Page Flush: Truncated message.");
        global_ops!(current_syscall_error._type = SEL4_TRUNCATED_MESSAGE);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    if unlikely(cap::cap_frame_cap(&cte.capability).get_capFMappedASID() == 0) {
        log::error!("[User] Page Flush: Frame is not mapped.");
        global_ops!(current_syscall_error._type = SEL4_ILLEGAL_OPERATION);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let _vaddr = cap::cap_frame_cap(&cte.capability).get_capFMappedAddress();
    let asid = cap::cap_frame_cap(&cte.capability).get_capFMappedASID() as usize;
    let find_ret = find_vspace_for_asid(asid);

    if unlikely(find_ret.status != exception_t::EXCEPTION_NONE) {
        log::error!("[User] Page Flush: No PGD for ASID");
        global_ops!(current_syscall_error._type = SEL4_FAILED_LOOKUP);
        global_ops!(current_syscall_error.failedLookupWasSource = 0);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let start = get_syscall_arg(0, buffer);
    let end = get_syscall_arg(1, buffer);

    if end <= start {
        log::error!("[User] Page Flush: Invalid range");
        global_ops!(current_syscall_error._type = SEL4_INVALID_ARGUMENT);
        global_ops!(current_syscall_error.invalidArgumentNumber = 1);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let page_size = BIT!(pageBitsForSize(
        cap::cap_frame_cap(&cte.capability).get_capFSize() as usize
    ));
    if start >= page_size || end > page_size {
        log::error!("[User] Page Flush: Requested range not inside page");
        global_ops!(current_syscall_error._type = SEL4_INVALID_ARGUMENT);
        global_ops!(current_syscall_error.invalidArgumentNumber = 0);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    let pstart =
        pptr_to_paddr(cap::cap_frame_cap(&cte.capability).get_capFBasePtr() as usize + start);
    get_currenct_thread().set_state(ThreadState::ThreadStateRestart);

    if start < end {
        let root_switched = set_vm_root_for_flush(find_ret.vspace_root.unwrap() as _, asid);
        // log::warn!(
        //     "need to flush cache for decode_page_clean_invocation label: {:?}",
        //     label
        // );

        do_flush(label, start, end, pstart);
        if root_switched {
            get_currenct_thread()
                .set_vm_root()
                .expect("can't set vm root for decode_page_clean_invocation");
        }
    }
    exception_t::EXCEPTION_NONE

    /*
        static exception_t performPageFlush(int invLabel, vspace_root_t *vspaceRoot, asid_t asid,
                                    vptr_t start, vptr_t end, paddr_t pstart)
        {
            bool_t root_switched;
                if (start < end) {
                    root_switched = setVMRootForFlush(vspaceRoot, asid);
                    do_flush(invLabel, start, end, pstart);
                    if (root_switched) {
                        setVMRoot(NODE_STATE(ksCurThread));
                    }
                }
            return EXCEPTION_NONE;
        }
    */
    /*
        return performPageFlush(invLabel, find_ret.vspace_root, asid, vaddr + start, vaddr + end - 1,
                                pstart);
    */
}

fn decode_frame_invocation(
    label: MessageLabel,
    length: usize,
    frame_slot: &mut cte_t,
    call: bool,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    // sel4_common::println!("decode frame invocation {}",label as usize);
    match label {
        MessageLabel::ARMPageMap => decode_frame_map(length, frame_slot, buffer),
        MessageLabel::ARMPageUnmap => {
            set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
            invoke_page_unmap(frame_slot)
        }
        MessageLabel::ARMPageClean_Data
        | MessageLabel::ARMPageInvalidate_Data
        | MessageLabel::ARMPageCleanInvalidate_Data
        | MessageLabel::ARMPageUnify_Instruction => {
            decode_page_clean_invocation(label, length, frame_slot, call, buffer)
        }
        MessageLabel::ARMPageGetAddress => {
            set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
            invoke_page_get_address(
                cap::cap_frame_cap(&frame_slot.capability).get_capFBasePtr() as usize,
                call,
            )
        }
        _ => {
            debug!("invalid operation label:{:?}", label);
            unsafe {
                current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
            }
            exception_t::EXCEPTION_SYSCALL_ERROR
        }
    }
}

fn decode_asid_control(label: MessageLabel, length: usize, buffer: &seL4_IPCBuffer) -> exception_t {
    if unlikely(label != MessageLabel::ARMASIDControlMakePool) {
        global_ops!(current_syscall_error._type = SEL4_ILLEGAL_OPERATION);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    if unlikely(
        length < 2
            || global_ops!(current_extra_caps.excaprefs[0] == 0)
            || global_ops!(current_extra_caps.excaprefs[1] == 0),
    ) {
        global_ops!(current_syscall_error._type = SEL4_TRUNCATED_MESSAGE);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    let index = get_syscall_arg(0, buffer);
    let depth = get_syscall_arg(1, buffer);
    let parent_slot =
        convert_to_mut_type_ref::<cte_t>(global_ops!(current_extra_caps.excaprefs[0]));
    let untyped = cap::cap_untyped_cap(&parent_slot.capability);
    let root =
        &convert_to_mut_type_ref::<cte_t>(global_ops!(current_extra_caps.excaprefs[1])).capability;

    let mut i = 0;
    loop {
        if !(i < N_ASID_POOLS && get_asid_pool_by_index(i) != 0) {
            break;
        }
        i += 1;
    }
    if unlikely(i == N_ASID_POOLS) {
        /* If no unallocated pool is found */
        global_ops!(current_syscall_error._type = SEL4_DELETE_FIRST);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    let asid_base = i << ASID_LOW_BITS;
    if unlikely(
        untyped.clone().unsplay().get_tag() != cap_tag::cap_untyped_cap
            || untyped.get_capBlockSize() as usize != SEL4_ASID_POOL_BITS
            || untyped.get_capIsDevice() == 1,
    ) {
        global_ops!(current_syscall_error._type = SEL4_INVALID_CAPABILITY);
        global_ops!(current_syscall_error.invalidCapNumber = 0);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    let status = parent_slot.ensure_no_children();
    if unlikely(status != exception_t::EXCEPTION_NONE) {
        return status;
    }
    let frame = untyped.get_capPtr() as usize;
    let lu_ret = lookup_slot_for_cnode_op(false, root, index, depth);
    if unlikely(lu_ret.status != exception_t::EXCEPTION_NONE) {
        return lu_ret.status;
    }
    let dest_slot = ptr_to_mut(lu_ret.slot);
    let status = ensure_empty_slot(dest_slot);
    if unlikely(status != exception_t::EXCEPTION_NONE) {
        return status;
    }
    get_currenct_thread().set_state(ThreadState::ThreadStateRestart);
    cap::cap_untyped_cap(&parent_slot.capability).set_capFreeIndex(max_free_index(
        cap::cap_untyped_cap(&parent_slot.capability).get_capBlockSize() as usize,
    ) as u64);
    unsafe {
        core::slice::from_raw_parts_mut(frame as *mut u8, BIT!(SEL4_ASID_POOL_BITS)).fill(0);
    }
    cte_insert(
        &cap_asid_pool_cap::new(asid_base as u64, frame as u64).unsplay(),
        parent_slot,
        dest_slot,
    );
    assert!(asid_base & MASK!(ASID_LOW_BITS) == 0);
    set_asid_pool_by_index(asid_base >> ASID_LOW_BITS, frame);
    exception_t::EXCEPTION_NONE
}

fn decode_asid_pool(label: MessageLabel, cte: &mut cte_t) -> exception_t {
    if unlikely(label != MessageLabel::ARMASIDPoolAssign) {
        global_ops!(current_syscall_error._type = SEL4_ILLEGAL_OPERATION);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    if unlikely(global_ops!(current_extra_caps.excaprefs[0] == 0)) {
        global_ops!(current_syscall_error._type = SEL4_TRUNCATED_MESSAGE);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let vspace_cap_slot = global_ops!(current_extra_caps.excaprefs[0]);
    let vspace_cap = convert_to_mut_type_ref::<cap_vspace_cap>(vspace_cap_slot);

    if unlikely(
        !vspace_cap.clone().unsplay().is_vtable_root() || vspace_cap.get_capVSIsMapped() == 1,
    ) {
        log::debug!("is not a valid vtable root");
        global_ops!(current_syscall_error._type = SEL4_INVALID_CAPABILITY);
        global_ops!(current_syscall_error.invalidArgumentNumber = 1);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    let pool = get_asid_pool_by_index(
        cap::cap_asid_pool_cap(&cte.capability).get_capASIDBase() as usize >> ASID_LOW_BITS,
    );

    if unlikely(pool == 0) {
        global_ops!(current_syscall_error._type = SEL4_FAILED_LOOKUP);
        global_ops!(current_syscall_error.failedLookupWasSource = 0);
        unsafe {
            current_lookup_fault = lookup_fault_invalid_root::new().unsplay();
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    if unlikely(pool != cap::cap_asid_pool_cap(&cte.capability).get_capASIDPool() as usize) {
        global_ops!(current_syscall_error._type = SEL4_INVALID_CAPABILITY);
        global_ops!(current_syscall_error.invalidCapNumber = 0);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    let mut asid = cap::cap_asid_pool_cap(&cte.capability).get_capASIDBase() as usize;
    let pool = convert_to_mut_type_ref::<asid_pool_t>(pool);
    let mut i = 0;

    // TODO: Make pool judge more efficient and pretty.
    while i < BIT!(ASID_LOW_BITS) && (asid + i == 0 || pool[i].0.arr[0] != 0) {
        i += 1;
    }

    if i == BIT!(ASID_LOW_BITS) {
        unsafe {
            current_syscall_error._type = SEL4_DELETE_FIRST;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }

    asid += i;

    get_currenct_thread().set_state(ThreadState::ThreadStateRestart);
    vspace_cap.set_capVSMappedASID(asid as u64);
    vspace_cap.set_capVSIsMapped(1);
    let asidmap = asid_map_asid_map_vspace::new(vspace_cap.get_capVSBasePtr() as u64).unsplay();
    pool[asid & MASK!(ASID_LOW_BITS)] = asidmap;
    exception_t::EXCEPTION_NONE
}

fn decode_frame_map(length: usize, frame_slot: &mut cte_t, buffer: &seL4_IPCBuffer) -> exception_t {
    if length < 3 || get_extra_cap_by_index(0).is_none() {
        debug!("ARMPageMap: Truncated message.");
        global_ops!(current_syscall_error._type = SEL4_TRUNCATED_MESSAGE);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    let vaddr = get_syscall_arg(0, buffer);
    let attr = vm_attributes_t::from_word(get_syscall_arg(2, buffer));
    let vspace_root_cap = cap::cap_vspace_cap(&get_extra_cap_by_index(0).unwrap().capability);
    let frame_size = cap::cap_frame_cap(&frame_slot.capability).get_capFSize() as usize;
    let frame_vm_rights = unsafe {
        core::mem::transmute(cap::cap_frame_cap(&frame_slot.capability).get_capFVMRights())
    };
    let vm_rights = maskVMRights(
        frame_vm_rights,
        seL4_CapRights(Bitfield {
            arr: [get_syscall_arg(1, buffer) as u64; 1],
        }),
    );
    if unlikely(!vspace_root_cap.clone().unsplay().is_valid_native_root()) {
        global_ops!(current_syscall_error._type = SEL4_INVALID_CAPABILITY);
        global_ops!(current_syscall_error.invalidCapNumber = 1);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    let vspace_root = vspace_root_cap.get_capVSBasePtr() as usize;
    let asid = vspace_root_cap.get_capVSMappedASID() as usize;
    let find_ret = find_vspace_for_asid(asid);
    if unlikely(find_ret.status != exception_t::EXCEPTION_NONE) {
        global_ops!(current_syscall_error._type = SEL4_FAILED_LOOKUP);
        global_ops!(current_syscall_error.failedLookupWasSource = 0);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    if unlikely(find_ret.vspace_root.unwrap() as usize != vspace_root) {
        global_ops!(current_syscall_error._type = SEL4_INVALID_CAPABILITY);
        global_ops!(current_syscall_error.invalidCapNumber = 1);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    if unlikely(!IS_ALIGNED!(vaddr, pageBitsForSize(frame_size))) {
        // global_var!(current_syscall_error)._type = SEL4_ALIGNMENT_ERROR;
        // Use unsafe here will cause the _type error.
        global_ops!(current_syscall_error._type = SEL4_ALIGNMENT_ERROR);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    let frame_asid = cap::cap_frame_cap(&frame_slot.capability).get_capFMappedASID() as usize;
    if frame_asid != ASID_INVALID {
        if frame_asid != asid {
            log::error!("[User] ARMPageMap: Attempting to remap a frame that does not belong to the passed address space");
            global_ops!(current_syscall_error._type = SEL4_INVALID_CAPABILITY);
            global_ops!(current_syscall_error.invalidArgumentNumber = 0);
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        } else if cap::cap_frame_cap(&frame_slot.capability).get_capFMappedAddress() as usize
            != vaddr
        {
            log::error!("[User] ARMPageMap: Attempting to map frame into multiple addresses");
            global_ops!(current_syscall_error._type = SEL4_INVALID_ARGUMENT);
            global_ops!(current_syscall_error.invalidArgumentNumber = 2);
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }
    } else {
        if unlikely(vaddr + BIT!(pageBitsForSize(frame_size)) - 1 > USER_TOP) {
            global_ops!(current_syscall_error._type = SEL4_INVALID_ARGUMENT);
            global_ops!(current_syscall_error.invalidArgumentNumber = 0);
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }
    }
    let mut vspace_root_pte = PTE::new_from_pte(vspace_root);
    let base = pptr_to_paddr(cap::cap_frame_cap(&frame_slot.capability).get_capFBasePtr() as usize);
    let lu_ret = vspace_root_pte.lookup_pt_slot(vaddr);
    if unlikely(lu_ret.ptBitsLeft != pageBitsForSize(frame_size)) {
        unsafe {
            current_lookup_fault =
                lookup_fault_missing_capability::new(lu_ret.ptBitsLeft as u64).unsplay();
            current_syscall_error._type = SEL4_FAILED_LOOKUP;
            current_syscall_error.failedLookupWasSource = 0;
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }
    }
    let pt_slot = convert_to_mut_type_ref::<PTE>(lu_ret.ptSlot as usize);
    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    cap::cap_frame_cap(&frame_slot.capability).set_capFMappedASID(asid as u64);
    cap::cap_frame_cap(&frame_slot.capability).set_capFMappedAddress(vaddr as u64);
    return invoke_page_map(
        asid,
        cap::cap_frame_cap(&frame_slot.capability.clone()).clone(),
        PTE::make_user_pte(base, vm_rights, attr, frame_size),
        pt_slot,
    );
    // match frame_size {
    //     ARM_SMALL_PAGE => {
    //         let lu_ret = vspace_root.lookup_pt_slot(vaddr);
    //         if lu_ret.status != exception_t::EXCEPTION_NONE {
    //             unsafe {
    //                 current_syscall_error._type = SEL4_FAILED_LOOKUP;
    //                 current_syscall_error.failedLookupWasSource = 0;
    //             }
    //             return exception_t::EXCEPTION_SYSCALL_ERROR;
    //         }
    //         set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    //         let ptSlot = convert_to_mut_type_ref::<PTE>(lu_ret.ptSlot as usize);
    //         invoke_small_page_map(
    //             vaddr,
    //             asid,
    //             frame_slot,
    //             makeUser3rdLevel(base, vm_rights, attr),
    //             ptSlot,
    //         )
    //     }
    //     ARM_LARGE_PAGE => {
    //         let lu_ret = vspace_root.lookup_pd_slot(vaddr);
    //         if lu_ret.status != exception_t::EXCEPTION_NONE {
    //             unsafe {
    //                 current_syscall_error._type = SEL4_FAILED_LOOKUP;
    //                 current_syscall_error.failedLookupWasSource = 0;
    //             }
    //             return exception_t::EXCEPTION_SYSCALL_ERROR;
    //         }
    //         set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    //         let pdSlot = convert_to_mut_type_ref::<PDE>(lu_ret.pdSlot as usize);
    //         invoke_large_page_map(
    //             vaddr,
    //             asid,
    //             frame_slot,
    //             make_user_2nd_level(base, vm_rights, attr),
    //             pdSlot,
    //         )
    //     }
    //     ARM_HUGE_PAGE => {
    //         let lu_ret = vspace_root.lookup_pud_slot(vaddr);
    //         if lu_ret.status != exception_t::EXCEPTION_NONE {
    //             unsafe {
    //                 current_syscall_error._type = SEL4_FAILED_LOOKUP;
    //                 current_syscall_error.failedLookupWasSource = 0;
    //             }
    //             return exception_t::EXCEPTION_SYSCALL_ERROR;
    //         }
    //         set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    //         let pudSlot = convert_to_mut_type_ref::<PUDE>(lu_ret.pudSlot as usize);
    //         invoke_huge_page_map(
    //             vaddr,
    //             asid,
    //             frame_slot,
    //             make_user_1st_level(base, vm_rights, attr),
    //             pudSlot,
    //         )
    //     }
    // _ => exception_t::EXCEPTION_SYSCALL_ERROR,
    // }
    // if length < 3 || get_extra_cap_by_index(0).is_none() {
    //     debug!("ARMPageMap: Truncated message.");
    //     unsafe {
    //         current_syscall_error._type = SEL4_TRUNCATED_MESSAGE;
    //     }
    //     return exception_t::EXCEPTION_SYSCALL_ERROR;
    // }
    // let vaddr = get_syscall_arg(0, buffer);
    // log::debug!("map frame: {:#x?}  frame: {:#x?}", frame_slot.cap.get_frame_mapped_address(), vaddr);
    // let attr = vm_attributes_t::from_word(get_syscall_arg(2, buffer));
    // let lvl1pt_cap = get_extra_cap_by_index(0).unwrap().cap;
    // let frame_size = frame_slot.cap.get_frame_size();
    // let frame_vm_rights = unsafe { core::mem::transmute(frame_slot.cap.get_frame_vm_rights()) };
    // let vm_rights = maskVMRights(
    //     frame_vm_rights,
    //     seL4_CapRights_t::from_word(get_syscall_arg(1, buffer)),
    // );
    // let (vspace_root, asid) = match get_vspace(&lvl1pt_cap) {
    //     Some(v) => v,
    //     _ => return exception_t::EXCEPTION_SYSCALL_ERROR,
    // };
    // if unlikely(!check_vp_alignment(frame_size, vaddr)) {
    //     unsafe {
    //         current_syscall_error._type = SEL4_ALIGNMENT_ERROR;
    //     }
    //     return exception_t::EXCEPTION_SYSCALL_ERROR;
    // }
    // let frame_asid = frame_slot.cap.get_frame_mapped_asid();
    // log::debug!("frame_asid: {:?}", frame_asid);
    // if frame_asid != ASID_INVALID {
    //     if frame_asid != asid {
    //         debug!("ARMPageMap: Attempting to remap a frame that does not belong to the passed address space");
    //         unsafe {
    //             current_syscall_error._type = SEL4_INVALID_CAPABILITY;
    //             current_syscall_error.invalidArgumentNumber = 0;
    //         }
    //         return exception_t::EXCEPTION_SYSCALL_ERROR;
    //     }
    //     if frame_slot.cap.get_frame_mapped_address() != vaddr {
    //         debug!("ARMPageMap: attempting to map frame into multiple addresses");
    //         unsafe {
    //             current_syscall_error._type = SEL4_INVALID_ARGUMENT;
    //             current_syscall_error.invalidArgumentNumber = 2;
    //         }
    //         return exception_t::EXCEPTION_SYSCALL_ERROR;
    //     }
    // } else {
    //     let vtop = vaddr + BIT!(pageBitsForSize(frame_size)) - 1;
    //     if unlikely(vtop >= USER_TOP) {
    //         unsafe {
    //             current_syscall_error._type = SEL4_INVALID_ARGUMENT;
    //             current_syscall_error.invalidArgumentNumber = 0;
    //         }
    //         return exception_t::EXCEPTION_SYSCALL_ERROR;
    //     }
    // }

    // // frame_slot.cap.set_frame_mapped_address(vaddr);
    // // frame_slot.cap.set_frame_mapped_asid(asid);

    // let base = pptr_to_paddr(frame_slot.cap.get_frame_base_ptr());
    // if frame_size == ARM_SMALL_PAGE {
    //     let lu_ret = vspace_root.lookup_pt_slot(vaddr);
    //     if lu_ret.status != exception_t::EXCEPTION_NONE {
    //         unsafe {
    //             current_syscall_error._type = SEL4_FAILED_LOOKUP;
    //             current_syscall_error.failedLookupWasSource = 0;
    //         }
    //         return exception_t::EXCEPTION_SYSCALL_ERROR;
    //     }
    //     set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    //     let ptSlot = convert_to_mut_type_ref::<PTE>(lu_ret.ptSlot as usize);
    //     invoke_small_page_map(
    //         vaddr,
    //         asid,
    //         frame_slot,
    //         makeUser3rdLevel(base, vm_rights, attr),
    //         ptSlot,
    //     )
    // } else if frame_size == ARM_LARGE_PAGE {
    //     let lu_ret = vspace_root.lookup_pd_slot(vaddr);
    //     if lu_ret.status != exception_t::EXCEPTION_NONE {
    //         unsafe {
    //             current_syscall_error._type = SEL4_FAILED_LOOKUP;
    //             current_syscall_error.failedLookupWasSource = 0;
    //         }
    //         return exception_t::EXCEPTION_SYSCALL_ERROR;
    //     }
    //     set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    //     let pdSlot = convert_to_mut_type_ref::<PDE>(lu_ret.pdSlot as usize);
    //     invoke_large_page_map(
    //         vaddr,
    //         asid,
    //         frame_slot,
    //         make_user_2nd_level(base, vm_rights, attr),
    //         pdSlot,
    //     )
    // } else if frame_size == ARM_HUGE_PAGE {
    //     let lu_ret = vspace_root.lookup_pud_slot(vaddr);
    //     if lu_ret.status != exception_t::EXCEPTION_NONE {
    //         unsafe {
    //             current_syscall_error._type = SEL4_FAILED_LOOKUP;
    //             current_syscall_error.failedLookupWasSource = 0;
    //         }
    //         return exception_t::EXCEPTION_SYSCALL_ERROR;
    //     }
    //     set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    //     let pudSlot = convert_to_mut_type_ref::<PUDE>(lu_ret.pudSlot as usize);
    //     invoke_huge_page_map(
    //         vaddr,
    //         asid,
    //         frame_slot,
    //         make_user_1st_level(base, vm_rights, attr),
    //         pudSlot,
    //     )
    // } else {
    //     return exception_t::EXCEPTION_SYSCALL_ERROR;
    // }
}

#[allow(unused)]
fn decode_page_table_unmap(pt_cte: &mut cte_t) -> exception_t {
    if !pt_cte.is_final_cap() {
        debug!("PageTableUnmap: cannot unmap if more than once cap exists");
        global_ops!(current_syscall_error._type = SEL4_REVOKE_FIRST);
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    let capability = &mut cap::cap_page_table_cap(&pt_cte.capability);
    // todo: in riscv here exists some more code ,but I don't know what it means and cannot find it in sel4,need check
    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);

    return invoke_page_table_unmap(capability);
}

// fn decode_upper_page_directory_unmap(ctSlot: &mut cte_t) -> exception_t {
//     let cap = &mut ctSlot.cap;
//     if cap.get_pud_is_mapped() != 0 {
//         let pud = &mut PUDE(cap.get_pud_base_ptr());
//         // TODO:: llh implement unmap_page_upper_directory as PUDE's method , but below two lines code both will cause sel4test end panic
//         pud.unmap_page_upper_directory(cap.get_pud_mapped_asid(), cap.get_pud_mapped_address());
//         // unmap_page_upper_directory(cap.get_pud_mapped_asid(), cap.get_pud_mapped_address(), pud);
//         clear_memory_pt(pud.self_addr() as *mut u8, cap.get_cap_size_bits());
//     }
//     cap.set_pud_is_mapped(0);
//     exception_t::EXCEPTION_NONE
// }

// fn decode_page_directory_unmap(ctSlot: &mut cte_t) -> exception_t {
//     let cap = &mut ctSlot.cap;
//     if cap.get_pd_is_mapped() != 0 {
//         let pd = &mut PDE(cap.get_pud_base_ptr());
//         // clear_memory(ptr, bits);
//         // TODO:: llh implement unmap_page_upper_directory as PUDE's method , but below two lines code both will cause sel4test end panic
//         pd.unmap_page_directory(cap.get_pd_mapped_asid(), cap.get_pd_mapped_address());
//         // unmap_page_directory(cap.get_pd_mapped_asid(), cap.get_pd_mapped_address(), pd);
//         clear_memory_pt(pd.self_addr() as *mut u8, cap.get_cap_size_bits());
//     }
//     cap.set_pud_is_mapped(0);
//     exception_t::EXCEPTION_NONE
// }

fn decode_vspace_root_invocation(
    label: MessageLabel,
    length: usize,
    cte: &mut cte_t,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    match label {
        MessageLabel::ARMVSpaceClean_Data
        | MessageLabel::ARMVSpaceInvalidate_Data
        | MessageLabel::ARMVSpaceCleanInvalidate_Data
        | MessageLabel::ARMVSpaceUnify_Instruction => {
            if length < 2 {
                debug!("VSpaceRoot Flush: Truncated message.");
                unsafe {
                    current_syscall_error._type = SEL4_TRUNCATED_MESSAGE;
                    return exception_t::EXCEPTION_SYSCALL_ERROR;
                }
            }
            let start = get_syscall_arg(0, buffer);
            let end = get_syscall_arg(1, buffer);
            if end <= start {
                debug!("VSpaceRoot Flush: Invalid range.");
                unsafe {
                    current_syscall_error._type = SEL4_INVALID_ARGUMENT;
                    current_syscall_error.invalidArgumentNumber = 1;
                    return exception_t::EXCEPTION_SYSCALL_ERROR;
                }
            }
            if end > USER_TOP {
                debug!("VSpaceRoot Flush: Exceed the user addressable region.");
                unsafe { current_syscall_error._type = SEL4_ILLEGAL_OPERATION };
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
            if !cte.capability.is_valid_native_root() {
                unsafe {
                    current_syscall_error._type = SEL4_INVALID_CAPABILITY;
                    current_syscall_error.invalidCapNumber = 0
                };
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
            let vspace_root = cap::cap_vspace_cap(&cte.capability).get_capVSBasePtr() as *mut PTE;
            let asid = cap::cap_asid_pool_cap(&cte.capability).get_capASIDBase() as usize;
            let find_ret = find_vspace_for_asid(asid);
            if find_ret.status != exception_t::EXCEPTION_NONE {
                debug!("VSpaceRoot Flush: No VSpace for ASID");
                unsafe {
                    current_syscall_error._type = SEL4_FAILED_LOOKUP;
                    current_syscall_error.failedLookupWasSource = 0;
                    return exception_t::EXCEPTION_SYSCALL_ERROR;
                }
            }
            if find_ret.vspace_root.unwrap() as usize != ptr_to_ref(vspace_root).get_ptr() {
                debug!("VSpaceRoot Flush: Invalid VSpace Cap");
                unsafe {
                    current_syscall_error._type = SEL4_INVALID_CAPABILITY;
                    current_syscall_error.invalidCapNumber = 0;
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
            let resolve_ret = ptr_to_mut(vspace_root).lookup_pt_slot(start);
            let pte = resolve_ret.ptSlot;
            if ptr_to_ref(pte).get_type() != (pte_tag_t::pte_page) as usize {
                get_currenct_thread().set_state(ThreadState::ThreadStateRestart);
                return exception_t::EXCEPTION_NONE;
            }
            let page_base_start = start & !MASK!(pageBitsForSize(resolve_ret.ptBitsLeft));
            let page_base_end = (end - 1) & !MASK!(pageBitsForSize(resolve_ret.ptBitsLeft));
            if page_base_start != page_base_end {
                unsafe {
                    current_syscall_error._type = SEL4_RANGE_ERROR;
                    current_syscall_error.rangeErrorMin = start;
                    current_syscall_error.rangeErrorMax =
                        page_base_start + MASK!(pageBitsForSize(resolve_ret.ptBitsLeft));
                }
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
            let pstart = ptr_to_ref(pte).get_page_base_address() + start
                & MASK!(pageBitsForSize(resolve_ret.ptBitsLeft));
            get_currenct_thread().set_state(ThreadState::ThreadStateRestart);
            return decode_vspace_flush_invocation(
                label,
                find_ret.vspace_root.unwrap() as usize,
                asid,
                start,
                end,
                pstart,
            );
        }
        _ => {
            unsafe { current_syscall_error._type = SEL4_ILLEGAL_OPERATION };
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }
    }
}

fn decode_vspace_flush_invocation(
    label: MessageLabel,
    vspace: usize,
    asid: asid_t,
    start: vptr_t,
    end: vptr_t,
    pstart: usize,
) -> exception_t {
    if start < end {
        let root_switched = set_vm_root_for_flush(vspace, asid);
        do_flush(label, start, end, pstart);
        if root_switched {
            let _ = get_currenct_thread().set_vm_root();
        }
    }
    exception_t::EXCEPTION_NONE
}

// fn decode_page_upper_directory_invocation(
//     label: MessageLabel,
//     length: usize,
//     cte: &mut cte_t,
//     buffer: &seL4_IPCBuffer,
// ) -> exception_t {
//     /*
//         lookupPGDSlot_ret_t pgdSlot;
//         findVSpaceForASID_ret_t find_ret;
//         if (invLabel == ARMPageUpperDirectoryUnmap) {
//             if (unlikely(!isFinalCapability(cte))) {
//                 current_syscall_error.type = SEL4_REVOKE_FIRST;
//                 return EXCEPTION_SYSCALL_ERROR;
//             }
//             set_thread_state(NODE_STATE(ksCurThread), ThreadState_Restart);
//             return performUpperPageDirectoryInvocationUnmap(cap, cte);
//         }
//     */
//     if label == MessageLabel::ARMPageUpperDirectoryUnmap {
//         // log::warn!("Need to check is FinalCapability here");
//         if unlikely(!cte.is_final_cap()) {
//             global_ops!(current_syscall_error._type = SEL4_REVOKE_FIRST);
//             return exception_t::EXCEPTION_SYSCALL_ERROR;
//         }
//         get_currenct_thread().set_state(ThreadState::ThreadStateRestart);
//         // unimplemented!("performUpperPageDirectoryInvocationUnmap");
//         return decode_upper_page_directory_unmap(cte);
//     }

//     // Return SYSCALL_ERROR if message is not ARMPageUpperDirectoryUnmap
//     if unlikely(label != MessageLabel::ARMPageUpperDirectoryMap) {
//         global_ops!(current_syscall_error._type = SEL4_ILLEGAL_OPERATION);
//         return exception_t::EXCEPTION_SYSCALL_ERROR;
//     }

//     if unlikely(length < 2 || unsafe { current_extra_caps.excaprefs[0] == 0 }) {
//         global_ops!(current_syscall_error._type = SEL4_TRUNCATED_MESSAGE);
//         return exception_t::EXCEPTION_SYSCALL_ERROR;
//     }
//     if unlikely(cte.cap.get_pud_is_mapped() == 1) {
//         global_ops!(current_syscall_error._type = SEL4_INVALID_CAPABILITY);
//         global_ops!(current_syscall_error.invalidCapNumber = 0);
//         return exception_t::EXCEPTION_SYSCALL_ERROR;
//     }
//     let vaddr = get_syscall_arg(0, buffer) & (!MASK!(PGD_INDEX_OFFSET));
//     let pgd_cap = convert_to_mut_type_ref::<cap_t>(global_ops!(current_extra_caps.excaprefs[0]));

//     if unlikely(!pgd_cap.is_valid_native_root()) {
//         global_ops!(current_syscall_error._type = SEL4_INVALID_CAPABILITY);
//         global_ops!(current_syscall_error.invalidCapNumber = 1);
//         return exception_t::EXCEPTION_SYSCALL_ERROR;
//     }

//     let pgd = pgd_cap.get_pgd_base_ptr();
//     let asid = pgd_cap.get_pgd_mapped_asid();

//     if unlikely(vaddr > USER_TOP) {
//         global_ops!(current_syscall_error._type = SEL4_INVALID_ARGUMENT);
//         global_ops!(current_syscall_error.failedLookupWasSource = 0);
//         return exception_t::EXCEPTION_SYSCALL_ERROR;
//     }

//     let find_ret = find_vspace_for_asid(asid);

//     if unlikely(find_ret.status != exception_t::EXCEPTION_NONE) {
//         global_ops!(current_syscall_error._type = SEL4_FAILED_LOOKUP);
//         global_ops!(current_syscall_error.failedLookupWasSource = 0);
//         return exception_t::EXCEPTION_SYSCALL_ERROR;
//     }
//     // vspace_root is Some(_) when Exception is NONE
//     if unlikely(find_ret.vspace_root.unwrap() as usize != pgd) {
//         global_ops!(current_syscall_error._type = SEL4_INVALID_CAPABILITY);
//         global_ops!(current_syscall_error.invalidCapNumber = 1);
//         return exception_t::EXCEPTION_SYSCALL_ERROR;
//     }
//     // Ensure that pgd is aligned 4K.
//     assert!(pgd & MASK!(PAGE_BITS) == 0);

//     let pgd_slot = PGDE::new_from_pte(pgd).lookup_pgd_slot(vaddr);

//     if unlikely(ptr_to_ref(pgd_slot.pgdSlot).get_present()) {
//         global_ops!(current_syscall_error._type = SEL4_DELETE_FIRST);
//         return exception_t::EXCEPTION_SYSCALL_ERROR;
//     }
//     // TODO: make 0x3 in a pagetable-specific position
//     let pgde = PGDE::new_page(pptr_to_paddr(cte.cap.get_pud_base_ptr()), 0x3);
//     cte.cap.set_pud_is_mapped(1);
//     cte.cap.set_pud_mapped_asid(asid);
//     cte.cap.set_pud_mapped_address(vaddr);

//     get_currenct_thread().set_state(ThreadState::ThreadStateRestart);
//     *ptr_to_mut(pgd_slot.pgdSlot) = pgde;
//     clean_by_va_pou(
//         convert_ref_type_to_usize(ptr_to_mut(pgd_slot.pgdSlot)),
//         pptr_to_paddr(convert_ref_type_to_usize(ptr_to_mut(pgd_slot.pgdSlot))),
//     );
//     exception_t::EXCEPTION_NONE
// }
// fn decode_page_directory_invocation(
//     label: MessageLabel,
//     length: usize,
//     cte: &mut cte_t,
//     buffer: &seL4_IPCBuffer,
// ) -> exception_t {
//     /*
//         if (invLabel == ARMPageDirectoryUnmap) {
//             if (unlikely(!isFinalCapability(cte))) {
//                 current_syscall_error.type = SEL4_REVOKE_FIRST;
//                 return EXCEPTION_SYSCALL_ERROR;
//             }
//             set_thread_state(NODE_STATE(ksCurThread), ThreadState_Restart);
//             return performPageDirectoryInvocationUnmap(cap, cte);
//         }
//     */
//     // Call performPageDirectoryInvocationUnmap if message is unmap
//     if label == MessageLabel::ARMPageDirectoryUnmap {
//         // log::warn!("Need to check is FinalCapability here");
//         if unlikely(!cte.is_final_cap()) {
//             global_ops!(current_syscall_error._type = SEL4_REVOKE_FIRST);
//             return exception_t::EXCEPTION_SYSCALL_ERROR;
//         }
//         get_currenct_thread().set_state(ThreadState::ThreadStateRestart);
//         // unimplemented!("performPageDirectoryInvocationUnmap");
//         return decode_page_directory_unmap(cte);
//     }

//     // Return SYSCALL_ERROR if message is not ARMPageDirectoryUnmap
//     if unlikely(label != MessageLabel::ARMPageDirectoryMap) {
//         global_ops!(current_syscall_error._type = SEL4_ILLEGAL_OPERATION);
//         return exception_t::EXCEPTION_SYSCALL_ERROR;
//     }
//     if unlikely(length < 2 || global_ops!(current_extra_caps.excaprefs[0] == 0)) {
//         global_ops!(current_syscall_error._type = SEL4_TRUNCATED_MESSAGE);
//         return exception_t::EXCEPTION_SYSCALL_ERROR;
//     }
//     if unlikely(cte.cap.get_pd_is_mapped() == 1) {
//         global_ops!(current_syscall_error._type = SEL4_INVALID_CAPABILITY);
//         global_ops!(current_syscall_error.invalidCapNumber = 0);
//         return exception_t::EXCEPTION_SYSCALL_ERROR;
//     }

//     let vaddr = get_syscall_arg(0, buffer) & (!MASK!(PUD_INDEX_OFFSET));
//     let vspace_root_cap =
//         convert_to_mut_type_ref::<cap_t>(global_ops!(current_extra_caps.excaprefs[0]));

//     if unlikely(!vspace_root_cap.is_valid_native_root()) {
//         global_ops!(current_syscall_error._type = SEL4_INVALID_CAPABILITY);
//         global_ops!(current_syscall_error.invalidCapNumber = 1);
//         return exception_t::EXCEPTION_SYSCALL_ERROR;
//     }

//     let vspace_root = vspace_root_cap.get_pgd_base_ptr();
//     let asid = vspace_root_cap.get_pgd_mapped_asid();

//     if unlikely(vaddr > USER_TOP) {
//         global_ops!(current_syscall_error._type = SEL4_INVALID_ARGUMENT);
//         global_ops!(current_syscall_error.failedLookupWasSource = 0);
//         return exception_t::EXCEPTION_SYSCALL_ERROR;
//     }

//     let find_ret = find_vspace_for_asid(asid);

//     if unlikely(find_ret.status != exception_t::EXCEPTION_NONE) {
//         global_ops!(current_syscall_error._type = SEL4_FAILED_LOOKUP);
//         global_ops!(current_syscall_error.failedLookupWasSource = 0);
//         return exception_t::EXCEPTION_SYSCALL_ERROR;
//     }
//     if unlikely(find_ret.vspace_root.unwrap() as usize != vspace_root) {
//         global_ops!(current_syscall_error._type = SEL4_INVALID_CAPABILITY);
//         global_ops!(current_syscall_error.invalidCapNumber = 1);
//         return exception_t::EXCEPTION_SYSCALL_ERROR;
//     }

//     let pud_slot = PGDE::new_from_pte(vspace_root).lookup_pud_slot(vaddr);

//     if pud_slot.status != exception_t::EXCEPTION_NONE {
//         global_ops!(current_syscall_error._type = SEL4_FAILED_LOOKUP);
//         global_ops!(current_syscall_error.failedLookupWasSource = 0);
//         return exception_t::EXCEPTION_SYSCALL_ERROR;
//     }
//     if unlikely(
//         ptr_to_ref(pud_slot.pudSlot).get_present() || ptr_to_ref(pud_slot.pudSlot).is_1g_page(),
//     ) {
//         global_ops!(current_syscall_error._type = SEL4_DELETE_FIRST);
//         return exception_t::EXCEPTION_SYSCALL_ERROR;
//     }
//     // TODO: make 0x3 in a pagetable-specific position
//     let pude = PUDE::new_page(pptr_to_paddr(cte.cap.get_pd_base_ptr()), 0x3);
//     cte.cap.set_pd_is_mapped(1);
//     cte.cap.set_pd_mapped_asid(asid);
//     cte.cap.set_pd_mapped_address(vaddr);
//     get_currenct_thread().set_state(ThreadState::ThreadStateRestart);
//     *ptr_to_mut(pud_slot.pudSlot) = pude;
//     // log::warn!("Need to clean D-Cache using cleanByVA_PoU");
//     clean_by_va_pou(
//         convert_ref_type_to_usize(ptr_to_mut(pud_slot.pudSlot)),
//         pptr_to_paddr(convert_ref_type_to_usize(ptr_to_mut(pud_slot.pudSlot))),
//     );
//     exception_t::EXCEPTION_NONE
// }

pub(crate) fn check_irq(irq: usize) -> exception_t {
    if irq > MAX_IRQ {
        unsafe {
            current_syscall_error._type = SEL4_RANGE_ERROR;
            current_syscall_error.rangeErrorMin = 0;
            current_syscall_error.rangeErrorMax = MAX_IRQ;
            debug!(
                "Rejecting request for IRQ {}. IRQ is out of range [1..MAX_IRQ].",
                irq
            );
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }
    }
    exception_t::EXCEPTION_NONE
}
#[cfg(not(feature = "enable_smp"))]
pub fn arch_decode_irq_control_invocation(
    label: MessageLabel,
    length: usize,
    src_slot: &mut cte_t,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    if label == MessageLabel::ARMIRQIssueIRQHandlerTrigger {
        if length < 4 || get_extra_cap_by_index(0).is_none() {
            unsafe {
                current_syscall_error._type = SEL4_TRUNCATED_MESSAGE;
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
                current_syscall_error._type = SEL4_REVOKE_FIRST;
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
            current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
}
#[cfg(feature = "enable_smp")]
pub fn arch_decode_irq_control_invocation(
    label: MessageLabel,
    length: usize,
    src_slot: &mut cte_t,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    if label == MessageLabel::ARMIRQIssueIRQHandlerTrigger {
        if length < 4 || get_extra_cap_by_index(0).is_none() {
            unsafe {
                current_syscall_error._type = SEL4_TRUNCATED_MESSAGE;
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
        if irq < NUM_PPI {
            unsafe {
                current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
            }
            debug!("Trying to get a handler on a PPI: use GetTriggerCore.");
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }
        if is_irq_active(irq) {
            unsafe {
                current_syscall_error._type = SEL4_REVOKE_FIRST;
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
    } else if label == MessageLabel::ARMIRQIssueIRQHandlerTriggerCore {
        let irq_w = get_syscall_arg(0, buffer);
        let _trigger = get_syscall_arg(1, buffer) != 0;
        let index = get_syscall_arg(2, buffer);
        let depth = get_syscall_arg(3, buffer) & 0xff;
        let target = get_syscall_arg(4, buffer);
        let cnode_cap = &get_extra_cap_by_index(0).unwrap().capability;
        let status = check_irq(irq_w);
        // let irq = to_irqt(irq_w, target);
        let irq_index = irq_to_idx(to_irqt(irq_w, target));
        let irq_irq = irqt_to_irq(to_irqt(irq_w, target));

        if status != exception_t::EXCEPTION_NONE {
            return status;
        }
        if target >= CONFIG_MAX_NUM_NODES {
            unsafe {
                current_syscall_error._type = SEL4_INVALID_ARGUMENT;
            }
            debug!("Target core {} is invalid.", target);
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }
        if is_irq_active(irq_index) {
            unsafe {
                current_syscall_error._type = SEL4_REVOKE_FIRST;
            }
            debug!("Rejecting request for IRQ {}. Already active.", irq_irq);
            return exception_t::EXCEPTION_SYSCALL_ERROR;
        }

        let lu_ret = lookupSlotForCNodeOp(false, cnode_cap, index, depth);
        if lu_ret.status != exception_t::EXCEPTION_NONE {
            debug!(
                "Target slot for new IRQ Handler cap invalid: IRQ {}.",
                irq_irq
            );
            return lu_ret.status;
        }

		let status = ensure_empty_slot(convert_to_mut_type_ref::<cte_t>(lu_ret.slot as usize));
		if status != exception_t::EXCEPTION_NONE {
			debug!(
                "Target slot for new IRQ Handler cap not empty: IRQ {}.",
				irq_irq
            );
			return status;
		}
        set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);

		if irq_w >= NUM_PPI {
			set_irq_target(irq_irq,target);
		}

        invoke_irq_control(
            irq_index,
            convert_to_mut_type_ref::<cte_t>(lu_ret.slot as usize),
            src_slot,
        )
    } else {
        unsafe {
            current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
}
#[cfg(feature = "enable_smc")]
pub fn decode_arm_smc_invocation(
    label: MessageLabel,
    length: usize,
    capability: &cap_smc_cap,
    call: bool,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    if label != ARMSMCCall {
        debug!("ARMSMCInvocation: Illegal operation.");
        unsafe {
            current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    if length < NUM_SMC_REGS {
        debug!("ARMSMCCall: Truncated message.");
        unsafe {
            current_syscall_error._type = SEL4_TRUNCATED_MESSAGE;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    let badge = capability.get_capSMCBadge();
    let smc_func_id = get_syscall_arg(0, buffer);
    if badge != 0 && badge != smc_func_id as u64 {
        debug!("ARMSMCCall: Illegal operation.");
        unsafe {
            current_syscall_error._type = SEL4_ILLEGAL_OPERATION;
        }
        return exception_t::EXCEPTION_SYSCALL_ERROR;
    }
    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);

    invoke_smc_call(buffer, call)
}
#[cfg(feature = "enable_smc")]
fn invoke_smc_call(buffer: &seL4_IPCBuffer, call: bool) -> exception_t {
    use core::arch::asm;

    let thread = get_currenct_thread();
    let op_ipc_buffer = thread.lookup_mut_ipc_buffer(true);

    let mut args: [usize; NUM_SMC_REGS] = [0; NUM_SMC_REGS];
    for i in 0..NUM_SMC_REGS {
        args[i] = get_syscall_arg(i, buffer);
    }
    unsafe {
        asm!(
            "mov x0, {0} \n",
            "mov x1, {1} \n",
            "mov x2, {2} \n",
            "mov x3, {3} \n",
            "mov x4, {4} \n",
            "mov x5, {5} \n",
            "mov x6, {6} \n",
            "mov x7, {7} \n",

            "smc #0 \n",
            "mov {0}, x0 \n",
            "mov {1}, x1 \n",
            "mov {2}, x2 \n",
            "mov {3}, x3 \n",
            "mov {4}, x4 \n",
            "mov {5}, x5 \n",
            "mov {6}, x6 \n",
            "mov {7}, x7 \n",
            inout(reg) args[0],
            inout(reg) args[1],
            inout(reg) args[2],
            inout(reg) args[3],
            inout(reg) args[4],
            inout(reg) args[5],
            inout(reg) args[6],
            inout(reg) args[7],
        );
    }
    if call {
        let mut i: usize = 0;
        while i < MSG_REGISTER_NUM {
            thread.tcbArch.set_register(ArchReg::Msg(i), args[i]);
            i += 1;
        }
        if let Some(ipc_buffer) = op_ipc_buffer {
            while i < NUM_SMC_REGS {
                ipc_buffer.msg[i + 1] = args[i];
                i += 1;
            }
        }
        thread.tcbArch.set_register(ArchReg::Badge, 0);
        thread.tcbArch.set_register(ArchReg::MsgInfo, 0);
    }
    set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRunning);
    exception_t::EXCEPTION_NONE
}
