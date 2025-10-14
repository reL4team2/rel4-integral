use crate::arch::fastpath_restore;
use crate::syscall::{slowpath, SYS_CALL, SYS_REPLY_RECV};
use core::intrinsics::{likely, unlikely};
#[cfg(feature = "kernel_mcs")]
use sched_context::sched_context_t;
use sel4_common::arch::MSG_REGISTER;
use sel4_common::message_info::seL4_MessageInfo_func;
use sel4_common::shared_types_bf_gen::seL4_MessageInfo;
#[cfg(feature = "kernel_mcs")]
use sel4_common::structures_gen::call_stack;
#[cfg(not(feature = "kernel_mcs"))]
use sel4_common::structures_gen::cap_reply_cap;
use sel4_common::structures_gen::{
    cap, cap_null_cap, cap_tag, endpoint, mdb_node, notification, seL4_Fault_tag, thread_state,
};
use sel4_common::{
    sel4_config::*,
    utils::{convert_to_mut_type_ref, convert_to_option_mut_type_ref},
};
use sel4_cspace::interface::*;
use sel4_ipc::*;
#[cfg(feature = "kernel_mcs")]
use sel4_task::reply::reply_t;
use sel4_task::*;
use sel4_vspace::*;

#[no_mangle]
pub fn lookup_fp(_cap: &cap, cptr: usize) -> cap {
    let mut capability = _cap.clone();
    let mut bits = 0;
    let mut guardBits: usize;
    let mut radixBits: usize;
    let mut cptr2: usize;
    let mut capGuard: usize;
    let mut radix: usize;
    let mut slot: *mut cte_t;
    if unlikely(!(capability.clone().get_tag() == cap_tag::cap_cnode_cap)) {
        return cap_null_cap::new().unsplay();
    }
    loop {
        let cnode_cap = cap::cap_cnode_cap(&capability);
        guardBits = cnode_cap.get_capCNodeGuardSize() as usize;
        radixBits = cnode_cap.get_capCNodeRadix() as usize;
        cptr2 = cptr << bits;
        capGuard = cnode_cap.get_capCNodeGuard() as usize;
        if likely(guardBits != 0) && unlikely(cptr2 >> (WORD_BITS - guardBits) != capGuard) {
            return cap_null_cap::new().unsplay();
        }

        radix = cptr2 << guardBits >> (WORD_BITS - radixBits);
        slot = unsafe { (cnode_cap.get_capCNodePtr() as *mut cte_t).add(radix) };
        capability = unsafe { (*slot).capability.clone() };
        bits += guardBits + radixBits;

        if likely(!(bits < WORD_BITS && capability.clone().get_tag() == cap_tag::cap_cnode_cap)) {
            break;
        }
    }
    if bits > WORD_BITS {
        return cap_null_cap::new().unsplay();
    }
    return capability;
}

#[no_mangle]
pub fn thread_state_ptr_mset_blockingObject_tsType(
    ptr: &mut thread_state,
    ep: usize,
    tsType: usize,
) {
    (*ptr).0.arr[0] = (ep | tsType) as u64;
}

#[no_mangle]
pub fn endpoint_ptr_mset_epQueue_tail_state(ptr: *mut endpoint, tail: usize, state: usize) {
    unsafe {
        (*ptr).0.arr[0] = (tail | state) as u64;
    }
}

#[no_mangle]
pub fn switch_to_thread_fp(thread: *mut tcb_t, vroot: *mut PTE, stored_hw_asid: PTE) {
    let asid = stored_hw_asid.0;
    unsafe {
        #[cfg(target_arch = "riscv64")]
        set_vspace_root(pptr!(vroot).to_paddr(), asid);
        #[cfg(target_arch = "aarch64")]
        set_current_user_vspace_root(ttbr_new(asid, pptr!(vroot).to_paddr()));
        // panic!("switch_to_thread_fp");
        // ksCurThread = thread as usize;
        set_current_thread(&*thread);
    }
}

#[no_mangle]
pub fn mdb_node_ptr_mset_mdbNext_mdbRevocable_mdbFirstBadged(
    ptr: &mut mdb_node,
    mdbNext: usize,
    mdbRevocable: usize,
    mdbFirstBadged: usize,
) {
    ptr.0.arr[1] = (mdbNext | (mdbRevocable << 1) | mdbFirstBadged) as u64;
}

#[no_mangle]
pub fn isValidVTableRoot_fp(capability: &cap) -> bool {
    // cap_capType_equals(cap, cap_page_table_cap) && cap.get_pt_is_mapped() != 0
    capability.get_tag() == cap_tag::cap_page_table_cap
        && cap::cap_page_table_cap(capability).get_capPTIsMapped() != 0
}

#[no_mangle]
pub fn fastpath_mi_check(msgInfo: usize) -> bool {
    (msgInfo & mask_bits!(SEL4_MSG_LENGTH_BITS + SEL4_MSG_EXTRA_CAP_BITS)) > 4
}

#[no_mangle]
pub fn fastpath_copy_mrs(length: usize, src: &mut tcb_t, dest: &mut tcb_t) {
    dest.tcbArch
        .copy_range(&src.tcbArch, MSG_REGISTER[0]..MSG_REGISTER[0] + length);
}

#[no_mangle]
pub fn fastpath_call(cptr: usize, msgInfo: usize) {
    // sel4_common::println!("fastpath call");
    let current = get_currenct_thread();
    let mut info = seL4_MessageInfo::from_word(msgInfo);
    let length = info.get_length() as usize;

    if fastpath_mi_check(msgInfo)
        || current.tcbFault.get_tag() != seL4_Fault_tag::seL4_Fault_NullFault
    {
        slowpath(SYS_CALL as usize);
    }
    let lookup_fp_ret = &lookup_fp(&current.get_cspace(TCB_CTABLE).capability, cptr);

    if unlikely(
        !(lookup_fp_ret.clone().get_tag() == cap_tag::cap_endpoint_cap)
            || (cap::cap_endpoint_cap(lookup_fp_ret).get_capCanSend() == 0),
    ) {
        slowpath(SYS_CALL as usize);
    }
    let ep_cap = cap::cap_endpoint_cap(lookup_fp_ret);
    let ep = convert_to_mut_type_ref::<endpoint>(ep_cap.get_capEPPtr() as usize);

    if unlikely(ep.get_ep_state() != EPState::Recv) {
        slowpath(SYS_CALL as usize);
    }

    let dest = convert_to_mut_type_ref::<tcb_t>(ep.get_epQueue_head() as usize);

    if unlikely(!isValidVTableRoot_fp(
        &dest.get_cspace(TCB_VTABLE).capability.clone(),
    )) {
        slowpath(SYS_CALL as usize);
    }
    let new_vtable = cap::cap_page_table_cap(&dest.get_cspace(TCB_VTABLE).capability);

    let dom = 0;
    if unlikely(dest.tcbPriority < current.tcbPriority && !is_highest_prio(dom, dest.tcbPriority)) {
        slowpath(SYS_CALL as usize);
    }
    if unlikely((ep_cap.get_capCanGrant() == 0) && (ep_cap.get_capCanGrantReply() == 0)) {
        slowpath(SYS_CALL as usize);
    }
    #[cfg(feature = "kernel_mcs")]
    {
        if unlikely(dest.tcbSchedContext != 0) {
            slowpath(SYS_CALL as usize);
        }
        assert!(dest.tcbState.get_tcbQueued() == 0);
        assert!(dest.tcbState.get_tcbInReleaseQueue() == 0);
        let reply = dest.tcbState.get_replyObject();
        if unlikely(reply == 0) {
            slowpath(SYS_CALL as usize);
        }
    }
    #[cfg(feature = "enable_smp")]
    if unlikely(get_currenct_thread().tcbAffinity != dest.tcbAffinity) {
        slowpath(SYS_CALL as usize);
    }

    // debug!("enter fast path");

    ep.set_epQueue_head(dest.tcbEPNext as u64);
    if unlikely(dest.tcbEPNext != 0) {
        convert_to_mut_type_ref::<tcb_t>(dest.tcbEPNext).tcbEPNext = 0;
    } else {
        ep.set_epQueue_tail(0);
        ep.set_state(EPState::Idle as u64);
    }

    current.tcbState.0.arr[0] = ThreadState::ThreadStateBlockedOnReply as u64;

    #[cfg(feature = "kernel_mcs")]
    {
        let reply = dest.tcbState.get_replyObject();
        assert!(dest.tcbState.get_tcbQueued() == 0);
        assert!(dest.tcbState.get_tcbInReleaseQueue() == 0);
        dest.tcbState.set_replyObject(0);

        assert!(current.tcbState.get_tcbQueued() == 0);
        assert!(current.tcbState.get_tcbInReleaseQueue() == 0);
        current.tcbState.set_replyObject(reply);

        convert_to_mut_type_ref::<reply_t>(reply as usize).replyTCB = NODE_STATE!(ksCurThread);

        let sc = convert_to_mut_type_ref::<sched_context_t>(current.tcbSchedContext);
        sc.scTcb = dest.get_ptr().raw();
        dest.tcbSchedContext = sc.get_ptr();
        current.tcbSchedContext = 0;

        convert_to_mut_type_ref::<reply_t>(reply as usize).replyPrev =
            call_stack::new(0, sc.scReply as u64);
        if unlikely(sc.scReply != 0) {
            convert_to_mut_type_ref::<reply_t>(sc.scReply).replyNext = call_stack::new(0, reply);
        }
        convert_to_mut_type_ref::<reply_t>(reply as usize).replyNext =
            call_stack::new(1, sc.get_ptr() as u64);
        sc.scReply = reply as usize;
    }
    #[cfg(not(feature = "kernel_mcs"))]
    {
        let reply_slot = current.get_cspace_mut_ref(TCB_REPLY);
        let caller_slot = dest.get_cspace_mut_ref(TCB_CALLER);
        let reply_can_grant = dest.tcbState.get_blockingIPCCanGrant();

        caller_slot.capability =
            cap_reply_cap::new(current.get_ptr().raw() as u64, reply_can_grant as u64, 0).unsplay();
        caller_slot.cteMDBNode.0.arr[0] = reply_slot.get_ptr() as u64;
        mdb_node_ptr_mset_mdbNext_mdbRevocable_mdbFirstBadged(
            &mut reply_slot.cteMDBNode,
            caller_slot.get_ptr(),
            1,
            1,
        );
    }

    fastpath_copy_mrs(length, current, dest);
    dest.tcbState.0.arr[0] = ThreadState::ThreadStateRunning as u64;
    let cap_pd = new_vtable.get_capPTBasePtr() as *mut PTE;
    let stored_hw_asid: PTE = PTE(new_vtable.get_capPTMappedASID() as usize);
    switch_to_thread_fp(dest as *mut tcb_t, cap_pd, stored_hw_asid);
    info.set_capsUnwrapped(0);
    let msgInfo1 = info.to_word();
    let badge = ep_cap.get_capEPBadge() as usize;
    fastpath_restore(badge, msgInfo1, get_currenct_thread());
}

#[no_mangle]
#[cfg(not(feature = "kernel_mcs"))]
pub fn fastpath_reply_recv(cptr: usize, msgInfo: usize) {
    // sel4_common::println!("enter fastpath_reply_recv");
    let current = get_currenct_thread();
    let mut info = seL4_MessageInfo::from_word(msgInfo);
    let length = info.get_length() as usize;
    let fault_type = current.tcbFault.get_tag();

    if fastpath_mi_check(msgInfo) || fault_type != seL4_Fault_tag::seL4_Fault_NullFault {
        slowpath(SYS_REPLY_RECV as usize);
    }
    let lookup_fp_ret = &lookup_fp(&current.get_cspace(TCB_CTABLE).capability, cptr);

    if unlikely(
        lookup_fp_ret.clone().get_tag() != cap_tag::cap_endpoint_cap
            || cap::cap_endpoint_cap(lookup_fp_ret).get_capCanSend() == 0,
    ) {
        slowpath(SYS_REPLY_RECV as usize);
    }
    let ep_cap = cap::cap_endpoint_cap(lookup_fp_ret);

    if let Some(ntfn) = convert_to_option_mut_type_ref::<notification>(current.tcbBoundNotification)
    {
        if ntfn.get_ntfn_state() == NtfnState::Active {
            slowpath(SYS_REPLY_RECV as usize);
        }
    }

    let ep = convert_to_mut_type_ref::<endpoint>(ep_cap.get_capEPPtr() as usize);
    if unlikely(ep.get_ep_state() == EPState::Send) {
        slowpath(SYS_REPLY_RECV as usize);
    }

    let caller_slot = current.get_cspace_mut_ref(TCB_CALLER);
    let caller_cap = &cap::cap_reply_cap(&caller_slot.capability);

    if unlikely(
        <cap_reply_cap as Clone>::clone(&caller_cap)
            .unsplay()
            .get_tag()
            != cap_tag::cap_reply_cap,
    ) {
        slowpath(SYS_REPLY_RECV as usize);
    }

    let caller = convert_to_mut_type_ref::<tcb_t>(caller_cap.get_capTCBPtr() as usize);
    if unlikely(caller.tcbFault.get_tag() != seL4_Fault_tag::seL4_Fault_NullFault) {
        slowpath(SYS_REPLY_RECV as usize);
    }

    if unlikely(!isValidVTableRoot_fp(
        &caller.get_cspace(TCB_VTABLE).capability.clone(),
    )) {
        slowpath(SYS_REPLY_RECV as usize);
    }
    let new_vtable = &cap::cap_page_table_cap(&caller.get_cspace(TCB_VTABLE).capability);

    let dom = 0;
    if unlikely(!is_highest_prio(dom, caller.tcbPriority)) {
        slowpath(SYS_REPLY_RECV as usize);
    }
    thread_state_ptr_mset_blockingObject_tsType(
        &mut current.tcbState,
        ep.get_ptr().raw(),
        ThreadState::ThreadStateBlockedOnReceive as usize,
    );
    current
        .tcbState
        .set_blockingIPCCanGrant(ep_cap.get_capCanGrant() as u64);

    if let Some(ep_tail_tcb) =
        convert_to_option_mut_type_ref::<tcb_t>(ep.get_epQueue_tail() as usize)
    {
        ep_tail_tcb.tcbEPNext = current.get_ptr().raw();
        current.tcbEPPrev = ep_tail_tcb.get_ptr().raw();
        current.tcbEPNext = 0;
    } else {
        current.tcbEPPrev = 0;
        current.tcbEPNext = 0;
        ep.set_epQueue_head(current.get_ptr().raw() as u64);
    }
    endpoint_ptr_mset_epQueue_tail_state(
        ep as *mut endpoint,
        get_currenct_thread().get_ptr().raw(),
        EPState_Recv,
    );

    // unsafe {
    let node = convert_to_mut_type_ref::<cte_t>(caller_slot.cteMDBNode.get_mdbPrev() as usize);
    mdb_node_ptr_mset_mdbNext_mdbRevocable_mdbFirstBadged(&mut node.cteMDBNode, 0, 1, 1);
    caller_slot.capability = cap_null_cap::new().unsplay();
    caller_slot.cteMDBNode = mdb_node::new(0, 0, 0, 0);
    fastpath_copy_mrs(length, current, caller);

    caller.tcbState.0.arr[0] = ThreadState::ThreadStateRunning as u64;
    let cap_pd = new_vtable.get_capPTBasePtr() as *mut PTE;
    let stored_hw_asid: PTE = PTE(new_vtable.get_capPTMappedASID() as usize);
    switch_to_thread_fp(caller, cap_pd, stored_hw_asid);
    info.set_capsUnwrapped(0);
    let msg_info1 = info.to_word();
    fastpath_restore(0, msg_info1, get_currenct_thread() as *mut tcb_t);
    // }
}

#[inline]
#[no_mangle]
#[cfg(feature = "kernel_mcs")]
pub fn fastpath_reply_recv(cptr: usize, msgInfo: usize, reply: usize) {
    // sel4_common::println!("enter fastpath_reply_recv");

    let current = get_currenct_thread();
    let mut info = seL4_MessageInfo::from_word(msgInfo);
    let length = info.get_length() as usize;
    let fault_type = current.tcbFault.get_tag();

    if fastpath_mi_check(msgInfo) || fault_type != seL4_Fault_tag::seL4_Fault_NullFault {
        slowpath(SYS_REPLY_RECV as usize);
    }
    let lookup_fp_ret = &lookup_fp(&current.get_cspace(TCB_CTABLE).capability, cptr);

    if unlikely(
        lookup_fp_ret.clone().get_tag() != cap_tag::cap_endpoint_cap
            || cap::cap_endpoint_cap(lookup_fp_ret).get_capCanSend() == 0,
    ) {
        slowpath(SYS_REPLY_RECV as usize);
    }
    let ep_cap = cap::cap_endpoint_cap(lookup_fp_ret);

    /* lookup the reply object */
    let lookup_fp_ret = &lookup_fp(&current.get_cspace(TCB_CTABLE).capability, reply);
    let reply_cap = cap::cap_reply_cap(lookup_fp_ret);

    /* check it's a reply object */
    if unlikely(reply_cap.clone().unsplay().get_tag() != cap_tag::cap_endpoint_cap) {
        slowpath(SYS_REPLY_RECV as usize);
    }

    if let Some(ntfn) = convert_to_option_mut_type_ref::<notification>(current.tcbBoundNotification)
    {
        if ntfn.get_ntfn_state() == NtfnState::Active {
            slowpath(SYS_REPLY_RECV as usize);
        }
    }

    let ep = convert_to_mut_type_ref::<endpoint>(ep_cap.get_capEPPtr() as usize);
    if unlikely(ep.get_ep_state() == EPState::Send) {
        slowpath(SYS_REPLY_RECV as usize);
    }
    /* Get the reply address */
    let reply_ptr = convert_to_mut_type_ref::<reply_t>(reply_cap.get_capReplyPtr() as usize);
    /* check that its valid and at the head of the call chain
    and that the current thread's SC is going to be donated. */
    if unlikely(
        reply_ptr.replyTCB == 0
            || reply_ptr.replyNext.get_isHead() == 0
            || reply_ptr.replyNext.get_callStackPtr() as usize != current.tcbSchedContext,
    ) {
        slowpath(SYS_REPLY_RECV as usize);
    }
    let caller = convert_to_mut_type_ref::<tcb_t>(reply_ptr.replyTCB);

    if unlikely(caller.tcbFault.get_tag() != seL4_Fault_tag::seL4_Fault_NullFault) {
        slowpath(SYS_REPLY_RECV as usize);
    }

    if unlikely(!isValidVTableRoot_fp(
        &caller.get_cspace(TCB_VTABLE).capability.clone(),
    )) {
        slowpath(SYS_REPLY_RECV as usize);
    }
    let new_vtable = cap::cap_page_table_cap(&caller.get_cspace(TCB_VTABLE).capability);

    let dom = 0;
    if unlikely(!is_highest_prio(dom, caller.tcbPriority)) {
        slowpath(SYS_REPLY_RECV as usize);
    }

    if unlikely(caller.tcbSchedContext != 0) {
        slowpath(SYS_REPLY_RECV as usize);
    }
    assert!(current.tcbState.get_replyObject() == 0);

    thread_state_ptr_mset_blockingObject_tsType(
        &mut current.tcbState,
        ep.get_ptr().raw(),
        ThreadState::ThreadStateBlockedOnReceive as usize,
    );
    caller.tcbState.set_replyObject(0);
    current
        .tcbState
        .set_replyObject(reply_cap.get_capReplyPtr());
    reply_ptr.replyTCB = current.get_ptr().raw();
    // #else
    //     thread_state_ptr_set_blockingIPCCanGrant(&NODE_STATE(ksCurThread)->tcbState,
    //                                              cap_endpoint_cap_get_capCanGrant(ep_cap));;
    // #endif
    // current
    //     .tcbState
    //     .set_blockingIPCCanGrant(ep_cap.get_capCanGrant() as u64);

    if let Some(_ep_tail_tcb) =
        convert_to_option_mut_type_ref::<tcb_t>(ep.get_epQueue_tail() as usize)
    {
        let mut queue = ep.get_queue();
        queue.ep_append(current);
        ep.set_epQueue_head(queue.head as u64);
        endpoint_ptr_mset_epQueue_tail_state(ep as *mut endpoint, queue.tail, EPState_Recv);
    } else {
        current.tcbEPPrev = 0;
        current.tcbEPNext = 0;
        ep.set_epQueue_head(current.get_ptr().as_u64());
        endpoint_ptr_mset_epQueue_tail_state(
            ep as *mut endpoint,
            get_currenct_thread().get_ptr().raw(),
            EPState_Recv,
        );
    }

    // #ifdef CONFIG_KERNEL_MCS
    //     /* update call stack */
    //     word_t prev_ptr = call_stack_get_callStackPtr(reply_ptr->replyPrev);
    //     sched_context_t *sc = NODE_STATE(ksCurThread)->tcbSchedContext;
    //     NODE_STATE(ksCurThread)->tcbSchedContext = NULL;
    //     caller->tcbSchedContext = sc;
    //     sc->scTcb = caller;

    //     sc->scReply = REPLY_PTR(prev_ptr);
    //     if (unlikely(REPLY_PTR(prev_ptr) != NULL)) {
    //         sc->scReply->replyNext = reply_ptr->replyNext;
    //     }

    //     /* TODO neccessary? */
    //     reply_ptr->replyPrev.words[0] = 0;
    //     reply_ptr->replyNext.words[0] = 0;
    let prev_ptr = reply_ptr.replyPrev.get_callStackPtr() as usize;
    let sc = current.tcbSchedContext;
    current.tcbSchedContext = 0;
    caller.tcbSchedContext = sc;
    let schedcontext = convert_to_mut_type_ref::<sched_context_t>(sc);
    schedcontext.scTcb = reply_ptr.replyTCB;
    schedcontext.scReply = prev_ptr;
    if unlikely(prev_ptr != 0) {
        let screply_ptr = convert_to_mut_type_ref::<reply_t>(schedcontext.scReply);
        screply_ptr.replyNext = reply_ptr.replyNext.clone();
    }
    reply_ptr.replyPrev.0.arr[0] = 0;
    reply_ptr.replyNext.0.arr[0] = 0;
    // #else
    //     /* Delete the reply cap. */
    //     mdb_node_ptr_mset_mdbNext_mdbRevocable_mdbFirstBadged(
    //         &CTE_PTR(mdb_node_get_mdbPrev(callerSlot->cteMDBNode))->cteMDBNode,
    //         0, 1, 1);
    //     callerSlot->cap = cap_null_cap_new();
    //     callerSlot->cteMDBNode = nullMDBNode;
    // #endif
    // unsafe {
    // let node = convert_to_mut_type_ref::<cte_t>(caller_slot.cteMDBNode.get_mdbPrev() as usize);
    // mdb_node_ptr_mset_mdbNext_mdbRevocable_mdbFirstBadged(&mut node.cteMDBNode, 0, 1, 1);
    // caller_slot.capability = cap_null_cap::new().unsplay();
    // caller_slot.cteMDBNode = mdb_node::new(0, 0, 0, 0);

    fastpath_copy_mrs(length, current, caller);

    caller.tcbState.0.arr[0] = ThreadState::ThreadStateRunning as u64;
    let cap_pd = new_vtable.get_capPTBasePtr() as *mut PTE;
    let stored_hw_asid: PTE = PTE(new_vtable.get_capPTMappedASID() as usize);
    switch_to_thread_fp(caller, cap_pd, stored_hw_asid);
    info.set_capsUnwrapped(0);
    let msg_info1 = info.to_word();
    fastpath_restore(0, msg_info1, get_currenct_thread() as *mut tcb_t);
    // }
}
