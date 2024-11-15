use crate::MASK;
use crate::{
    config::seL4_MsgLengthBits,
    syscall::{slowpath, SysCall, SysReplyRecv},
};
use core::intrinsics::{likely, unlikely};
use sel4_common::arch::msgRegister;
use sel4_common::message_info::seL4_MessageInfo_func;
use sel4_common::shared_types_bf_gen::seL4_MessageInfo;
use sel4_common::structures_gen::{
    cap, cap_cnode_cap, cap_null_cap, cap_page_table_cap, cap_reply_cap, cap_tag, endpoint,
    mdb_node, notification, seL4_Fault_tag, thread_state,
};
use sel4_common::{
    sel4_config::*,
    utils::{convert_to_mut_type_ref, convert_to_option_mut_type_ref},
};
use sel4_cspace::interface::*;
use sel4_ipc::*;
use sel4_task::*;
use sel4_vspace::*;

#[inline]
#[no_mangle]
pub fn lookup_fp(_cap: &cap_cnode_cap, cptr: usize) -> cap {
    let mut capability = _cap.clone();
    let mut bits = 0;
    let mut guardBits: usize;
    let mut radixBits: usize;
    let mut cptr2: usize;
    let mut capGuard: usize;
    let mut radix: usize;
    let mut slot: *mut cte_t;
    if unlikely(!(capability.clone().unsplay().get_tag() == cap_tag::cap_cnode_cap)) {
        return cap_null_cap::new().unsplay();
    }
    loop {
        guardBits = capability.get_capCNodeGuardSize() as usize;
        radixBits = capability.get_capCNodeRadix() as usize;
        cptr2 = cptr << bits;
        capGuard = capability.get_capCNodeGuard() as usize;
        if likely(guardBits != 0) && unlikely(cptr2 >> (wordBits - guardBits) != capGuard) {
            return cap_null_cap::new().unsplay();
        }

        radix = cptr2 << guardBits >> (wordBits - radixBits);
        slot = unsafe { (capability.get_capCNodePtr() as *mut cte_t).add(radix) };
        capability = unsafe { cap::cap_cnode_cap(&(*slot).capability).clone() };
        bits += guardBits + radixBits;

        if likely(
            !(bits < wordBits && capability.clone().unsplay().get_tag() == cap_tag::cap_cnode_cap),
        ) {
            break;
        }
    }
    if bits > wordBits {
        return cap_null_cap::new().unsplay();
    }
    return capability.unsplay();
}

#[inline]
#[no_mangle]
pub fn thread_state_ptr_mset_blockingObject_tsType(
    ptr: &mut thread_state,
    ep: usize,
    tsType: usize,
) {
    (*ptr).0.arr[0] = (ep | tsType) as u64;
}

#[inline]
#[no_mangle]
pub fn endpoint_ptr_mset_epQueue_tail_state(ptr: *mut endpoint, tail: usize, state: usize) {
    unsafe {
        (*ptr).0.arr[0] = (tail | state) as u64;
    }
}

#[inline]
#[no_mangle]
pub fn switchToThread_fp(thread: *mut tcb_t, vroot: *mut PTE, stored_hw_asid: PTE) {
    let asid = stored_hw_asid.0;
    unsafe {
        #[cfg(target_arch = "riscv64")]
        setVSpaceRoot(pptr_to_paddr(vroot as usize), asid);
        #[cfg(target_arch = "aarch64")]
        setCurrentUserVSpaceRoot(ttbr_new(asid, pptr_to_paddr(vroot as usize)));
        // panic!("switchToThread_fp");
        // ksCurThread = thread as usize;
        set_current_thread(&*thread);
    }
}

#[inline]
#[no_mangle]
pub fn mdb_node_ptr_mset_mdbNext_mdbRevocable_mdbFirstBadged(
    ptr: &mut mdb_node,
    mdbNext: usize,
    mdbRevocable: usize,
    mdbFirstBadged: usize,
) {
    ptr.0.arr[1] = (mdbNext | (mdbRevocable << 1) | mdbFirstBadged) as u64;
}

#[inline]
#[no_mangle]
pub fn isValidVTableRoot_fp(capability: &cap) -> bool {
    // cap_capType_equals(cap, cap_page_table_cap) && cap.get_pt_is_mapped() != 0
    capability.get_tag() == cap_tag::cap_page_table_cap
        && cap::cap_page_table_cap(capability).get_capPTIsMapped() != 0
}

#[inline]
#[no_mangle]
pub fn fastpath_mi_check(msgInfo: usize) -> bool {
    (msgInfo & MASK!(seL4_MsgLengthBits + seL4_MsgExtraCapBits)) > 4
}

#[inline]
#[no_mangle]
pub fn fastpath_copy_mrs(length: usize, src: &mut tcb_t, dest: &mut tcb_t) {
    dest.tcbArch
        .copy_range(&src.tcbArch, msgRegister[0]..msgRegister[0] + length);
}

// #[inline]
// #[no_mangle]
// pub fn fastpath_restore(badge: usize, msgInfo: usize, cur_thread: *mut tcb_t) {
//     let cur_thread_regs = unsafe { (*cur_thread).tcbArch.get_register().as_ptr() as usize };
//     extern "C" {
//         pub fn __restore_fp(badge: usize, msgInfo: usize, cur_thread_reg: usize);
//         fn fastpath_restore(badge: usize, msgInfo: usize, cur_thread: usize);
//     }
//     unsafe {
//         __restore_fp(badge, msgInfo, cur_thread_regs);
//     }
// }
#[inline]
#[no_mangle]
#[cfg(target_arch = "aarch64")]
pub fn fastpath_restore(_badge: usize, _msgInfo: usize, cur_thread: *mut tcb_t) {
    use core::arch::asm;
    unsafe {
        (*cur_thread).tcbArch.load_thread_local();
        asm!(
            "mov     sp, {}                     \n",
            /* Restore thread's SPSR, LR, and SP */
            "ldp     x21, x22, [sp, #31 * 8]  \n",
            "ldr     x23, [sp, #33 * 8]     \n",
            "msr     sp_el0, x21                \n",
            // #ifdef CONFIG_ARM_HYPERVISOR_SUPPORT
            // 		"msr     elr_el2, x22               \n"
            // 		"msr     spsr_el2, x23              \n"
            // #else
            "msr     elr_el1, x22               \n",
            "msr     spsr_el1, x23              \n",
            // #endif

            /* Restore remaining registers */
            "ldp     x2,  x3,  [sp, #16 * 1]    \n",
            "ldp     x4,  x5,  [sp, #16 * 2]    \n",
            "ldp     x6,  x7,  [sp, #16 * 3]    \n",
            "ldp     x8,  x9,  [sp, #16 * 4]    \n",
            "ldp     x10, x11, [sp, #16 * 5]    \n",
            "ldp     x12, x13, [sp, #16 * 6]    \n",
            "ldp     x14, x15, [sp, #16 * 7]    \n",
            "ldp     x16, x17, [sp, #16 * 8]    \n",
            "ldp     x18, x19, [sp, #16 * 9]    \n",
            "ldp     x20, x21, [sp, #16 * 10]   \n",
            "ldp     x22, x23, [sp, #16 * 11]   \n",
            "ldp     x24, x25, [sp, #16 * 12]   \n",
            "ldp     x26, x27, [sp, #16 * 13]   \n",
            "ldp     x28, x29, [sp, #16 * 14]   \n",
            "ldr     x30, [sp, #30 * 8]           \n",
            "eret                                 ",
            in(reg) (*cur_thread).tcbArch.raw_ptr()
        );
    }
    panic!("unreachable")
}

#[inline]
#[no_mangle]
#[cfg(target_arch = "riscv64")]
pub fn fastpath_restore(_badge: usize, _msgInfo: usize, cur_thread: *mut tcb_t) {
    #[cfg(feature = "ENABLE_SMP")]
    {}
    extern "C" {
        pub fn __fastpath_restore(badge: usize, msgInfo: usize, cur_thread_reg: usize);
    }
    unsafe {
        __fastpath_restore(_badge, _msgInfo, (*cur_thread).tcbArch.raw_ptr());
    }
    panic!("unreachable")
}

#[inline]
#[no_mangle]
pub fn fastpath_call(cptr: usize, msgInfo: usize) {
    let current = get_currenct_thread();
    let mut info = seL4_MessageInfo::from_word(msgInfo);
    let length = info.get_length() as usize;

    if fastpath_mi_check(msgInfo)
        || current.tcbFault.get_tag() != seL4_Fault_tag::seL4_Fault_NullFault
    {
        slowpath(SysCall as usize);
    }
    let lookup_fp_ret = &lookup_fp(
        &cap::cap_cnode_cap(&current.get_cspace(tcbCTable).capability),
        cptr,
    );
    let ep_cap = cap::cap_endpoint_cap(lookup_fp_ret);
    if unlikely(
        !(ep_cap.clone().unsplay().get_tag() == cap_tag::cap_endpoint_cap)
            || (ep_cap.get_capCanSend() == 0),
    ) {
        slowpath(SysCall as usize);
    }
    let ep = convert_to_mut_type_ref::<endpoint>(ep_cap.get_capEPPtr() as usize);

    if unlikely(ep.get_ep_state() != EPState::Recv) {
        slowpath(SysCall as usize);
    }

    let dest = convert_to_mut_type_ref::<tcb_t>(ep.get_epQueue_head() as usize);
    let new_vtable = cap::cap_page_table_cap(&dest.get_cspace(tcbVTable).capability);

    if unlikely(!isValidVTableRoot_fp(&new_vtable.clone().unsplay())) {
        slowpath(SysCall as usize);
    }

    let dom = 0;
    if unlikely(dest.tcbPriority < current.tcbPriority && !isHighestPrio(dom, dest.tcbPriority)) {
        slowpath(SysCall as usize);
    }
    if unlikely((ep_cap.get_capCanGrant() == 0) && (ep_cap.get_capCanGrantReply() == 0)) {
        slowpath(SysCall as usize);
    }
    // #ifdef CONFIG_KERNEL_MCS
    //     if (unlikely(dest->tcbSchedContext != NULL)) {
    //         slowpath(SysCall);
    //     }

    //     reply_t *reply = thread_state_get_replyObject_np(dest->tcbState);
    //     if (unlikely(reply == NULL)) {
    //         slowpath(SysCall);
    //     }
    // #endif
    #[cfg(feature = "ENABLE_SMP")]
    if unlikely(get_currenct_thread().tcbAffinity != dest.tcbAffinity) {
        slowpath(SysCall as usize);
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

    #[cfg(feature = "KERNEL_MCS")]
    {
        // TODO: MCS
        // #ifdef CONFIG_KERNEL_MCS
        //     thread_state_ptr_set_replyObject_np(&dest->tcbState, 0);
        //     thread_state_ptr_set_replyObject_np(&NODE_STATE(ksCurThread)->tcbState, REPLY_REF(reply));
        //     reply->replyTCB = NODE_STATE(ksCurThread);

        //     sched_context_t *sc = NODE_STATE(ksCurThread)->tcbSchedContext;
        //     sc->scTcb = dest;
        //     dest->tcbSchedContext = sc;
        //     NODE_STATE(ksCurThread)->tcbSchedContext = NULL;

        //     reply_t *old_caller = sc->scReply;
        //     reply->replyPrev = call_stack_new(REPLY_REF(sc->scReply), false);
        //     if (unlikely(old_caller)) {
        //         old_caller->replyNext = call_stack_new(REPLY_REF(reply), false);
        //     }
        //     reply->replyNext = call_stack_new(SC_REF(sc), true);
        //     sc->scReply = reply;
        // #endif
    }
    #[cfg(not(feature = "KERNEL_MCS"))]
    {
        let reply_slot = current.get_cspace_mut_ref(tcbReply);
        let caller_slot = dest.get_cspace_mut_ref(tcbCaller);
        let reply_can_grant = dest.tcbState.get_blockingIPCCanGrant();

        caller_slot.capability =
            cap_reply_cap::new(current.get_ptr() as u64, reply_can_grant as u64, 0).unsplay();
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
    switchToThread_fp(dest as *mut tcb_t, cap_pd, stored_hw_asid);
    info.set_capsUnwrapped(0);
    let msgInfo1 = info.to_word();
    let badge = ep_cap.get_capEPBadge() as usize;
    fastpath_restore(badge, msgInfo1, get_currenct_thread());
}

#[inline]
#[no_mangle]
#[cfg(not(feature = "KERNEL_MCS"))]
pub fn fastpath_reply_recv(cptr: usize, msgInfo: usize) {
    // debug!("enter fastpath_reply_recv");
    let current = get_currenct_thread();
    let mut info = seL4_MessageInfo::from_word(msgInfo);
    let length = info.get_length() as usize;
    let fault_type = current.tcbFault.get_tag();

    if fastpath_mi_check(msgInfo) || fault_type != seL4_Fault_tag::seL4_Fault_NullFault {
        slowpath(SysReplyRecv as usize);
    }
    let lookup_fp_ret = &lookup_fp(
        &cap::cap_cnode_cap(&current.get_cspace(tcbCTable).capability),
        cptr,
    );
    let ep_cap = cap::cap_endpoint_cap(lookup_fp_ret);

    if unlikely(
        ep_cap.clone().unsplay().get_tag() != cap_tag::cap_endpoint_cap
            || ep_cap.get_capCanSend() == 0,
    ) {
        slowpath(SysReplyRecv as usize);
    }

    if let Some(ntfn) = convert_to_option_mut_type_ref::<notification>(current.tcbBoundNotification)
    {
        if ntfn.get_ntfn_state() == NtfnState::Active {
            slowpath(SysReplyRecv as usize);
        }
    }

    let ep = convert_to_mut_type_ref::<endpoint>(ep_cap.get_capEPPtr() as usize);
    if unlikely(ep.get_ep_state() == EPState::Send) {
        slowpath(SysReplyRecv as usize);
    }

    let caller_slot = current.get_cspace_mut_ref(tcbCaller);
    let caller_cap = &cap::cap_reply_cap(&caller_slot.capability);

    if unlikely(
        <cap_reply_cap as Clone>::clone(&caller_cap)
            .unsplay()
            .get_tag()
            != cap_tag::cap_reply_cap,
    ) {
        slowpath(SysReplyRecv as usize);
    }

    let caller = convert_to_mut_type_ref::<tcb_t>(caller_cap.get_capTCBPtr() as usize);
    if unlikely(caller.tcbFault.get_tag() != seL4_Fault_tag::seL4_Fault_NullFault) {
        slowpath(SysReplyRecv as usize);
    }

    let new_vtable = &cap::cap_page_table_cap(&caller.get_cspace(tcbVTable).capability);

    if unlikely(!isValidVTableRoot_fp(
        &<cap_page_table_cap as Clone>::clone(&new_vtable).unsplay(),
    )) {
        slowpath(SysReplyRecv as usize);
    }

    let dom = 0;
    if unlikely(!isHighestPrio(dom, caller.tcbPriority)) {
        slowpath(SysReplyRecv as usize);
    }
    thread_state_ptr_mset_blockingObject_tsType(
        &mut current.tcbState,
        ep.get_ptr(),
        ThreadState::ThreadStateBlockedOnReceive as usize,
    );
    current
        .tcbState
        .set_blockingIPCCanGrant(ep_cap.get_capCanGrant() as u64);

    if let Some(ep_tail_tcb) =
        convert_to_option_mut_type_ref::<tcb_t>(ep.get_epQueue_tail() as usize)
    {
        ep_tail_tcb.tcbEPNext = current.get_ptr();
        current.tcbEPPrev = ep_tail_tcb.get_ptr();
        current.tcbEPNext = 0;
    } else {
        current.tcbEPPrev = 0;
        current.tcbEPNext = 0;
        ep.set_epQueue_head(current.get_ptr() as u64);
    }
    endpoint_ptr_mset_epQueue_tail_state(
        ep as *mut endpoint,
        get_currenct_thread().get_ptr(),
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
    switchToThread_fp(caller, cap_pd, stored_hw_asid);
    info.set_capsUnwrapped(0);
    let msg_info1 = info.to_word();
    fastpath_restore(0, msg_info1, get_currenct_thread() as *mut tcb_t);
    // }
}

#[inline]
#[no_mangle]
#[cfg(feature = "KERNEL_MCS")]
pub fn fastpath_reply_recv(cptr: usize, msgInfo: usize, reply: usize) {
    // debug!("enter fastpath_reply_recv");

    use sel4_common::{reply::reply_t, sched_context::sched_context_t};
    use sel4_ipc::endpoint_func;
    let current = get_currenct_thread();
    let mut info = seL4_MessageInfo::from_word(msgInfo);
    let length = info.get_length() as usize;
    let fault_type = current.tcbFault.get_tag();

    if fastpath_mi_check(msgInfo) || fault_type != seL4_Fault_tag::seL4_Fault_NullFault {
        slowpath(SysReplyRecv as usize);
    }
    let lookup_fp_ret = &lookup_fp(
        &cap::cap_cnode_cap(&current.get_cspace(tcbCTable).capability),
        cptr,
    );
    let ep_cap = cap::cap_endpoint_cap(lookup_fp_ret);

    if unlikely(
        ep_cap.clone().unsplay().get_tag() != cap_tag::cap_endpoint_cap
            || ep_cap.get_capCanSend() == 0,
    ) {
        slowpath(SysReplyRecv as usize);
    }

    /* lookup the reply object */
    let lookup_fp_ret = &lookup_fp(
        &cap::cap_cnode_cap(&current.get_cspace(tcbCTable).capability),
        reply,
    );
    let reply_cap = cap::cap_reply_cap(lookup_fp_ret);

    /* check it's a reply object */
    if unlikely(reply_cap.clone().unsplay().get_tag() != cap_tag::cap_endpoint_cap) {
        slowpath(SysReplyRecv as usize);
    }

    if let Some(ntfn) = convert_to_option_mut_type_ref::<notification>(current.tcbBoundNotification)
    {
        if ntfn.get_ntfn_state() == NtfnState::Active {
            slowpath(SysReplyRecv as usize);
        }
    }

    let ep = convert_to_mut_type_ref::<endpoint>(ep_cap.get_capEPPtr() as usize);
    if unlikely(ep.get_ep_state() == EPState::Send) {
        slowpath(SysReplyRecv as usize);
    }
    // #ifdef CONFIG_KERNEL_MCS
    //     /* Get the reply address */
    //     reply_t *reply_ptr = REPLY_PTR(cap_reply_cap_get_capReplyPtr(reply_cap));
    //     /* check that its valid and at the head of the call chain
    //        and that the current thread's SC is going to be donated. */
    //     if (unlikely(reply_ptr->replyTCB == NULL ||
    //                  call_stack_get_isHead(reply_ptr->replyNext) == 0 ||
    //                  SC_PTR(call_stack_get_callStackPtr(reply_ptr->replyNext)) != NODE_STATE(ksCurThread)->tcbSchedContext)) {
    //         slowpath(SysReplyRecv);
    //     }

    //     /* Determine who the caller is. */
    //     caller = reply_ptr->replyTCB;
    // #endif
    /* Get the reply address */
    let reply_ptr = convert_to_mut_type_ref::<reply_t>(reply_cap.get_capReplyPtr() as usize);
    /* check that its valid and at the head of the call chain
    and that the current thread's SC is going to be donated. */
    if unlikely(
        reply_ptr.replyTCB == 0
            || reply_ptr.replyNext.get_isHead() == 0
            || reply_ptr.replyNext.get_callStackPtr() as usize != current.tcbSchedContext,
    ) {
        slowpath(SysReplyRecv as usize);
    }
    let caller = convert_to_mut_type_ref::<tcb_t>(reply_ptr.replyTCB);

    if unlikely(caller.tcbFault.get_tag() != seL4_Fault_tag::seL4_Fault_NullFault) {
        slowpath(SysReplyRecv as usize);
    }

    let new_vtable = cap::cap_page_table_cap(&caller.get_cspace(tcbVTable).capability);

    if unlikely(!isValidVTableRoot_fp(
        &<cap_page_table_cap as Clone>::clone(&new_vtable).unsplay(),
    )) {
        slowpath(SysReplyRecv as usize);
    }

    let dom = 0;
    if unlikely(!isHighestPrio(dom, caller.tcbPriority)) {
        slowpath(SysReplyRecv as usize);
    }

    if unlikely(caller.tcbSchedContext !=0){
        slowpath(SysReplyRecv as usize);
    }

    thread_state_ptr_mset_blockingObject_tsType(
        &mut current.tcbState,
        ep.get_ptr(),
        ThreadState::ThreadStateBlockedOnReceive as usize,
    );
    // #ifdef CONFIG_KERNEL_MCS
    //     /* unlink reply object from caller */
    //     thread_state_ptr_set_replyObject_np(&caller->tcbState, 0);
    //     /* set the reply object */
    //     thread_state_ptr_set_replyObject_np(&NODE_STATE(ksCurThread)->tcbState, REPLY_REF(reply_ptr));
    //     reply_ptr->replyTCB = NODE_STATE(ksCurThread);
    caller.tcbState.set_replyObject(0);
    current.tcbState.set_replyObject(reply_cap.get_capReplyPtr());
    reply_ptr.replyTCB = unsafe{current as *mut _ as usize};
    // #else
    //     thread_state_ptr_set_blockingIPCCanGrant(&NODE_STATE(ksCurThread)->tcbState,
    //                                              cap_endpoint_cap_get_capCanGrant(ep_cap));;
    // #endif
    // current
    //     .tcbState
    //     .set_blockingIPCCanGrant(ep_cap.get_capCanGrant() as u64);

    if let Some(ep_tail_tcb) =
        convert_to_option_mut_type_ref::<tcb_t>(ep.get_epQueue_tail() as usize)
    {
        let queue = unsafe{tcbEPAppend(current, ep.get_queue())};
        ep.set_epQueue_head(queue.head as u64);
        endpoint_ptr_mset_epQueue_tail_state(
            ep as *mut endpoint,
            queue.head,
            EPState_Recv,
        );
    } else {
        current.tcbEPPrev = 0;
        current.tcbEPNext = 0;
        ep.set_epQueue_head(current.get_ptr() as u64);
        endpoint_ptr_mset_epQueue_tail_state(
            ep as *mut endpoint,
            get_currenct_thread().get_ptr(),
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
    switchToThread_fp(caller, cap_pd, stored_hw_asid);
    info.set_capsUnwrapped(0);
    let msg_info1 = info.to_word();
    fastpath_restore(0, msg_info1, get_currenct_thread() as *mut tcb_t);
    // }
}
