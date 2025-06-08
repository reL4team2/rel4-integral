//! This module contains the implementation of the scheduler for the sel4_task crate.
//!
//! It includes functions and data structures related to task scheduling and thread management.
//! The scheduler supports Symmetric Multiprocessing (SMP) and provides functionality for choosing
//! new threads to run, managing ready queues, and handling domain scheduling.
//!
#![allow(unused_unsafe)]
#![allow(static_mut_ref)]

#[cfg(feature = "enable_smp")]
use crate::deps::do_mask_reschedule;
use core::arch::asm;
use core::intrinsics::{likely, unlikely};
use sel4_common::arch::ArchReg;
#[cfg(feature = "enable_smp")]
use sel4_common::sel4_config::CONFIG_MAX_NUM_NODES;
use sel4_common::sel4_config::{
    CONFIG_NUM_DOMAINS, CONFIG_NUM_PRIORITIES, CONFIG_TIME_SLICE, L2_BITMAP_SIZE, NUM_READY_QUEUES,
    TCB_OFFSET, WORD_BITS, WORD_RADIX,
};
#[cfg(feature = "enable_smp")]
use sel4_common::utils::cpu_id;
use sel4_common::utils::{convert_to_mut_type_ref, ptr_to_usize_add};
use sel4_common::{BIT, MASK};

use crate::deps::ksIdleThreadTCB;
#[cfg(feature = "kernel_mcs")]
use crate::sched_context::{sched_context_t, MIN_REFILLS};
use crate::tcb::{set_thread_state, tcb_t};
use crate::tcb_queue::tcb_queue_t;
use crate::thread_state::ThreadState;
#[cfg(feature = "kernel_mcs")]
use crate::{deps::ksIdleThreadSC, sched_context::refill_budget_check, tcb_release_dequeue};
#[cfg(feature = "kernel_mcs")]
use sel4_common::{
    arch::us_to_ticks,
    platform::time_def::{ticks_t, time_t, US_IN_MS},
    sel4_config::CONFIG_BOOT_THREAD_TIME_SLICE,
    utils::convert_to_mut_type_ref_unsafe,
};
#[cfg(target_arch = "aarch64")]
use sel4_vspace::{
    get_arm_global_user_vspace_base, kpptr_to_paddr, set_current_user_vspace_root, ttbr_new,
};

cfg_if::cfg_if! {
    if #[cfg(feature = "enable_smp")] {
        #[derive(Debug, Copy, Clone)]
        /// Struct representing the SMP (Symmetric Multiprocessing) state data.
        pub struct SmpStateData {
            /// Number of pending IPI (Inter-Processor Interrupt) reschedule requests.
            pub ipiReschedulePending: usize,
            /// Array of ready queues for each domain and priority level.
            pub ksReadyQueues: [tcb_queue_t; NUM_READY_QUEUES],
            /// Bitmap representing the presence of ready queues at the L1 level for each domain.
            pub ksReadyQueuesL1Bitmap: [usize; CONFIG_NUM_DOMAINS],
            /// Bitmap representing the presence of ready queues at the L2 level for each domain and priority level.
            pub ksReadyQueuesL2Bitmap: [[usize; L2_BITMAP_SIZE]; CONFIG_NUM_DOMAINS],
            /// Index of the currently executing thread.
            pub ksCurThread: usize,
            /// Index of the idle thread.
            pub ksIdleThread: usize,
            /// Action to be taken by the scheduler.
            pub ksSchedulerAction: usize,
            /// MCS relative field
            #[cfg(feature = "kernel_mcs")]
            pub ksReleaseQueue: tcb_queue_t,
            #[cfg(feature = "kernel_mcs")]
            pub ksConsumed: time_t,
            #[cfg(feature = "kernel_mcs")]
            pub ksCurTime: time_t,
            #[cfg(feature = "kernel_mcs")]
            pub ksReprogram: bool,
            #[cfg(feature = "kernel_mcs")]
            pub ksCurSC: usize,
            #[cfg(feature = "kernel_mcs")]
            pub ksIdleSC: usize,
            /// Number of debug TCBs (Thread Control Blocks).
            pub ksActiveFPUState: usize,
            // TODO: Cache Line 对齐
            pub ks_fpu_restore_since_switch: usize,
        }

        #[no_mangle]
        pub static mut ksSMP: [SmpStateData; CONFIG_MAX_NUM_NODES] = [SmpStateData {
            ipiReschedulePending: 0,
            ksReadyQueues: [tcb_queue_t { head: 0, tail: 0 }; NUM_READY_QUEUES],
            ksReadyQueuesL1Bitmap: [0; CONFIG_NUM_DOMAINS],
            ksReadyQueuesL2Bitmap: [[0; L2_BITMAP_SIZE]; CONFIG_NUM_DOMAINS],
            ksCurThread: 0,
            ksIdleThread: 0,
            ksSchedulerAction: 1,
            #[cfg(feature = "kernel_mcs")]
            ksReleaseQueue: tcb_queue_t { head: 0, tail: 0 },
            #[cfg(feature = "kernel_mcs")]
            ksConsumed: 0,
            #[cfg(feature = "kernel_mcs")]
            ksCurTime: 0,
            #[cfg(feature = "kernel_mcs")]
            ksReprogram: false,
            #[cfg(feature = "kernel_mcs")]
            ksCurSC: 0,
            #[cfg(feature = "kernel_mcs")]
            ksIdleSC: 0,
            ksActiveFPUState: 0,
            ks_fpu_restore_since_switch: 0,
        }; CONFIG_MAX_NUM_NODES];
    } else {
        #[no_mangle]
        pub static mut ksReadyQueues: [tcb_queue_t; NUM_READY_QUEUES] =
            [tcb_queue_t { head: 0, tail: 0 }; NUM_READY_QUEUES];

        #[no_mangle]
        pub static mut ksReadyQueuesL2Bitmap: [[usize; L2_BITMAP_SIZE]; CONFIG_NUM_DOMAINS] =
            [[0; L2_BITMAP_SIZE]; CONFIG_NUM_DOMAINS];

        #[no_mangle]
        pub static mut ksReadyQueuesL1Bitmap: [usize; CONFIG_NUM_DOMAINS] = [0; CONFIG_NUM_DOMAINS];

        #[no_mangle]
        pub static mut ksCurThread: usize = 0;

        #[no_mangle]
        pub static mut ksIdleThread: usize = 0;

        #[no_mangle]
        pub static mut ksSchedulerAction: usize = 1;

        #[cfg(feature = "have_fpu")]
        #[no_mangle]
        pub static mut ksActiveFPUState: usize = 0;

        #[cfg(feature = "have_fpu")]
        #[no_mangle]
        pub static mut ks_fpu_restore_since_switch: usize = 0;

        #[no_mangle]
        #[cfg(feature = "kernel_mcs")]
        pub static mut ksReleaseQueue: tcb_queue_t = tcb_queue_t { head: 0, tail: 0 };

        #[no_mangle]
        #[cfg(feature = "kernel_mcs")]
        pub static mut ksCurSC: usize = 0;

        #[no_mangle]
        #[cfg(feature = "kernel_mcs")]
        pub static mut ksConsumed: time_t = 0;

        #[no_mangle]
        #[cfg(feature = "kernel_mcs")]
        pub static mut ksCurTime: time_t = 0;

        #[no_mangle]
        #[cfg(feature = "kernel_mcs")]
        pub static mut ksReprogram: bool = false;

        #[no_mangle]
        #[cfg(feature = "kernel_mcs")]
        pub static mut ksIdleSC: usize = 0;
    }
}

#[repr(C)]
#[derive(Debug, PartialEq, Clone, Copy)]
/// Struct representing a domain schedule.
pub struct dschedule_t {
    /// Domain ID.
    pub domain: usize,
    /// Length of the domain schedule.
    pub length: usize,
}

pub const SCHEDULER_ACTION_RESUME_CURRENT_THREAD: usize = 0;
pub const SCHEDULER_ACTION_CHOOSE_NEW_THREAD: usize = 1;
pub const KS_DOM_SCHEDULE_LENGTH: usize = 1;

pub const SCHED_CONTEXT_NO_FLAGS: usize = 0;
pub const SCHED_CONTEXT_SPORADIC: usize = 1;
#[no_mangle]
pub static mut ksDomainTime: usize = 0;

#[no_mangle]
pub static mut ksCurDomain: usize = 0;

#[no_mangle]
pub static mut ksDomScheduleIdx: usize = 0;

#[no_mangle]
#[link_section = ".boot.bss"]
pub static mut ksWorkUnitsCompleted: usize = 0;

// #[link_section = ".boot.bss"]
pub static mut ksDomSchedule: [dschedule_t; KS_DOM_SCHEDULE_LENGTH] = [dschedule_t {
    domain: 0,
    length: 60,
}; KS_DOM_SCHEDULE_LENGTH];

#[allow(non_camel_case_types)]
pub type prio_t = usize;

/// seL4 NODE_STATE, get the current node state field
#[cfg(feature = "enable_smp")]
#[macro_export]
macro_rules! NODE_STATE {
    ($field:ident) => {
        unsafe { $crate::ksSMP[sel4_common::utils::cpu_id()].$field }
    };
}

#[cfg(not(feature = "enable_smp"))]
#[macro_export]
macro_rules! NODE_STATE {
    ($field:ident) => {
        unsafe { $crate::$field }
    };
}

/// seL4 NODE_STATE_ON_CORE, get the core node state field
#[cfg(feature = "enable_smp")]
#[macro_export]
macro_rules! NODE_STATE_ON_CORE {
    ($cpu:expr, $field:ident) => {
        unsafe { $crate::ksSMP[$cpu].$field }
    };
}

#[cfg(not(feature = "enable_smp"))]
#[macro_export]
macro_rules! NODE_STATE_ON_CORE {
    ($cpu:expr, $field:ident) => {
        unsafe { $crate::$field }
    };

    ($field:ident) => {
        unsafe { $crate::$field }
    };
}

/// SET_NODE_STATE, set the core node state field
#[cfg(feature = "enable_smp")]
#[macro_export]
macro_rules! SET_NODE_STATE {
    ($field:ident = $val:expr) => {
        unsafe {
            $crate::ksSMP[sel4_common::utils::cpu_id()].$field = $val;
        }
    };
}

#[cfg(not(feature = "enable_smp"))]
#[macro_export]
macro_rules! SET_NODE_STATE {
    ($field:ident = $val:expr) => {
        unsafe {
            $crate::$field = $val;
        }
    };
}

/// SET_NODE_STATE_ON_CORE, set the specific core node state field
#[cfg(feature = "enable_smp")]
#[macro_export]
macro_rules! SET_NODE_STATE_ON_CORE {
    ($cpu:expr, $field:ident = $val:expr) => {
        unsafe {
            $crate::ksSMP[$cpu].$field = $val;
        }
    };
}

#[cfg(not(feature = "enable_smp"))]
#[macro_export]
macro_rules! SET_NODE_STATE_ON_CORE {
    ($cpu:expr, $field:ident = $val:expr) => {
        unsafe {
            $crate::$field = $val;
        }
    };

    ($field:ident = $val:expr) => {
        unsafe {
            $crate::$field = $val;
        }
    };
}

#[inline]
/// Get the idle thread, and returns a mutable tcb reference to the idle thread.
pub fn get_idle_thread() -> &'static mut tcb_t {
    convert_to_mut_type_ref::<tcb_t>(NODE_STATE!(ksIdleThread))
}

#[inline]
/// Get the current thread, and returns a mutable tcb reference to the current thread.
/// FIXME: fix the name of this function, get_current_thread
pub fn get_currenct_thread() -> &'static mut tcb_t {
    convert_to_mut_type_ref::<tcb_t>(NODE_STATE!(ksCurThread))
}

#[inline]
pub fn get_current_thread_on_node(_node: usize) -> &'static mut tcb_t {
    convert_to_mut_type_ref::<tcb_t>(NODE_STATE_ON_CORE!(_node, ksCurThread))
}

#[inline]
/// Set the current thread.
pub fn set_current_thread(thread: &tcb_t) {
    SET_NODE_STATE!(ksCurThread = thread.get_ptr());
}

#[inline]
/// Get the current domain.
pub fn get_current_domain() -> usize {
    unsafe { ksCurDomain }
}

#[inline]
#[cfg(feature = "kernel_mcs")]
pub fn get_current_sc() -> &'static mut sched_context_t {
    convert_to_mut_type_ref_unsafe::<sched_context_t>(NODE_STATE!(ksCurSC))
}

#[inline]
/// Get the index of the ready queue for the given domain and priority level.
pub fn ready_queues_index(dom: usize, prio: usize) -> usize {
    dom * CONFIG_NUM_PRIORITIES + prio
}

#[inline]
/// Get the L1 index for the given priority level.
fn prio_to_l1index(prio: usize) -> usize {
    prio >> WORD_RADIX
}

#[inline]
/// Get the priority level for the given L1 index.
fn l1index_to_prio(l1index: usize) -> usize {
    l1index << WORD_RADIX
}

#[inline]
/// Invert the L1 index.
fn invert_l1index(l1index: usize) -> usize {
    let inverted = L2_BITMAP_SIZE - 1 - l1index;
    inverted
}

#[cfg(not(feature = "enable_smp"))]
#[inline]
/// Get the highest priority level for the given domain in single-core mode.
fn get_highest_prio(dom: usize) -> prio_t {
    unsafe {
        let l1index = WORD_BITS - 1 - ksReadyQueuesL1Bitmap[dom].leading_zeros() as usize;
        let l1index_inverted = invert_l1index(l1index);
        let l2index =
            WORD_BITS - 1 - ksReadyQueuesL2Bitmap[dom][l1index_inverted].leading_zeros() as usize;
        l1index_to_prio(l1index) | l2index
    }
}

#[cfg(feature = "enable_smp")]
#[inline]
/// Get the highest priority level for the given domain on the current CPU in multi-core mode.
fn get_highest_prio(dom: usize) -> prio_t {
    unsafe {
        let l1index =
            WORD_BITS - 1 - ksSMP[cpu_id()].ksReadyQueuesL1Bitmap[dom].leading_zeros() as usize;
        let l1index_inverted = invert_l1index(l1index);
        let l2index = WORD_BITS
            - 1
            - (ksSMP[cpu_id()].ksReadyQueuesL2Bitmap[dom])[l1index_inverted].leading_zeros()
                as usize;
        l1index_to_prio(l1index) | l2index
    }
}

#[inline]
/// Check if the given priority level is the highest priority level for the given domain.
pub fn is_highest_prio(dom: usize, prio: prio_t) -> bool {
    #[cfg(feature = "enable_smp")]
    {
        unsafe { ksSMP[cpu_id()].ksReadyQueuesL1Bitmap[dom] == 0 || prio >= get_highest_prio(dom) }
    }
    #[cfg(not(feature = "enable_smp"))]
    {
        unsafe { ksReadyQueuesL1Bitmap[dom] == 0 || prio >= get_highest_prio(dom) }
    }
}

#[inline]
/// Add the given priority level to the ready queue bitmap.
pub fn add_to_bitmap(_cpu: usize, dom: usize, prio: usize) {
    unsafe {
        let l1index = prio_to_l1index(prio);
        let l1index_inverted = invert_l1index(l1index);
        #[cfg(feature = "enable_smp")]
        {
            ksSMP[_cpu].ksReadyQueuesL1Bitmap[dom] |= BIT!(l1index);
            ksSMP[_cpu].ksReadyQueuesL2Bitmap[dom][l1index_inverted] |=
                BIT!(prio & MASK!(WORD_RADIX));
        }
        #[cfg(not(feature = "enable_smp"))]
        {
            ksReadyQueuesL1Bitmap[dom] |= BIT!(l1index);
            ksReadyQueuesL2Bitmap[dom][l1index_inverted] |= BIT!(prio & MASK!(WORD_RADIX));
        }
    }
}

#[inline]
/// Remove the given priority level from the ready queue bitmap.
pub fn remove_from_bigmap(_cpu: usize, dom: usize, prio: usize) {
    unsafe {
        let l1index = prio_to_l1index(prio);
        let l1index_inverted = invert_l1index(l1index);
        #[cfg(feature = "enable_smp")]
        {
            ksSMP[_cpu].ksReadyQueuesL2Bitmap[dom][l1index_inverted] &=
                !BIT!(prio & MASK!(WORD_RADIX));
            if unlikely(ksSMP[_cpu].ksReadyQueuesL2Bitmap[dom][l1index_inverted] == 0) {
                ksSMP[_cpu].ksReadyQueuesL1Bitmap[dom] &= !(BIT!((l1index)));
            }
        }
        #[cfg(not(feature = "enable_smp"))]
        {
            ksReadyQueuesL2Bitmap[dom][l1index_inverted] &= !BIT!(prio & MASK!(WORD_RADIX));
            if unlikely(ksReadyQueuesL2Bitmap[dom][l1index_inverted] == 0) {
                ksReadyQueuesL1Bitmap[dom] &= !(BIT!((l1index)));
            }
        }
    }
}

fn next_domain() {
    unsafe {
        ksDomScheduleIdx += 1;
        if ksDomScheduleIdx >= KS_DOM_SCHEDULE_LENGTH {
            ksDomScheduleIdx = 0;
        }
        #[cfg(feature = "kernel_mcs")]
        {
            SET_NODE_STATE!(ksReprogram = true);
        }
        ksWorkUnitsCompleted = 0;
        ksCurDomain = ksDomSchedule[ksDomScheduleIdx].domain;
        #[cfg(feature = "kernel_mcs")]
        {
            ksDomainTime = us_to_ticks(ksDomSchedule[ksDomScheduleIdx].length * US_IN_MS);
        }
        #[cfg(not(feature = "kernel_mcs"))]
        {
            ksDomainTime = ksDomSchedule[ksDomScheduleIdx].length;
        }
        //FIXME ksWorkUnits not used;
        // ksWorkUnits
    }
}

fn schedule_choose_new_thread() {
    // if hart_id() == 0 {
    //     debug!("schedule_choose_new_thread");
    // }

    unsafe {
        if ksDomainTime == 0 {
            next_domain();
        }
    }
    choose_thread();
}

fn choose_thread() {
    unsafe {
        let dom = 0;
        let ks_l1_bit = {
            #[cfg(feature = "enable_smp")]
            {
                ksSMP[cpu_id()].ksReadyQueuesL1Bitmap[dom]
            }
            #[cfg(not(feature = "enable_smp"))]
            {
                ksReadyQueuesL1Bitmap[dom]
            }
        };
        if likely(ks_l1_bit != 0) {
            let prio = get_highest_prio(dom);
            let thread = {
                #[cfg(feature = "enable_smp")]
                {
                    ksSMP[cpu_id()].ksReadyQueues[ready_queues_index(dom, prio)].head
                }
                #[cfg(not(feature = "enable_smp"))]
                {
                    ksReadyQueues[ready_queues_index(dom, prio)].head
                }
            };
            assert_ne!(thread, 0);
            assert!(convert_to_mut_type_ref::<tcb_t>(thread).is_schedulable());
            #[cfg(feature = "kernel_mcs")]
            {
                assert!(convert_to_mut_type_ref::<sched_context_t>(
                    convert_to_mut_type_ref::<tcb_t>(thread).tcbSchedContext
                )
                .refill_sufficient(0));
                assert!(convert_to_mut_type_ref::<sched_context_t>(
                    convert_to_mut_type_ref::<tcb_t>(thread).tcbSchedContext
                )
                .refill_ready());
            }
            convert_to_mut_type_ref::<tcb_t>(thread).switch_to_this();
        } else {
            #[cfg(target_arch = "aarch64")]
            {
                set_current_user_vspace_root(ttbr_new(
                    0,
                    kpptr_to_paddr(get_arm_global_user_vspace_base()),
                ));
                set_current_thread(get_idle_thread());
            }
            #[cfg(target_arch = "riscv64")]
            get_idle_thread().switch_to_this();
        }
    }
}

#[no_mangle]
#[cfg(not(feature = "kernel_mcs"))]
/// Reschedule threads, and enqueue the current thread if current ks scheduler action is not to resume the current thread and choose new thread.
pub fn reschedule_required() {
    if NODE_STATE!(ksSchedulerAction) != SCHEDULER_ACTION_RESUME_CURRENT_THREAD
        && NODE_STATE!(ksSchedulerAction) != SCHEDULER_ACTION_CHOOSE_NEW_THREAD
    {
        convert_to_mut_type_ref::<tcb_t>(NODE_STATE!(ksSchedulerAction)).sched_enqueue();
    }
    // ksSchedulerAction = SCHEDULER_ACTION_CHOOSE_NEW_THREAD;
    SET_NODE_STATE!(ksSchedulerAction = SCHEDULER_ACTION_CHOOSE_NEW_THREAD);
}
#[no_mangle]
#[cfg(feature = "kernel_mcs")]
/// Reschedule threads, and enqueue the current thread if current ks scheduler action is not to resume the current thread and choose new thread.
pub fn reschedule_required() {
    let action = NODE_STATE!(ksSchedulerAction);
    if action != SCHEDULER_ACTION_RESUME_CURRENT_THREAD
        && action != SCHEDULER_ACTION_CHOOSE_NEW_THREAD
    {
        let action_tcb = convert_to_mut_type_ref::<tcb_t>(action);
        if action_tcb.is_schedulable() {
            let action_sched_context =
                convert_to_mut_type_ref::<sched_context_t>(action_tcb.tcbSchedContext);
            assert!(action_sched_context.refill_sufficient(0));
            assert!(action_sched_context.refill_ready());
            action_tcb.sched_enqueue();
        }
    }
    SET_NODE_STATE!(ksSchedulerAction = SCHEDULER_ACTION_CHOOSE_NEW_THREAD);
}
#[cfg(feature = "kernel_mcs")]
pub fn awaken() {
    while unlikely(
        NODE_STATE!(ksReleaseQueue).head != 0
            && convert_to_mut_type_ref::<sched_context_t>(
                convert_to_mut_type_ref::<tcb_t>(NODE_STATE!(ksReleaseQueue).head).tcbSchedContext,
            )
            .refill_ready(),
    ) {
        let awakened = tcb_release_dequeue();
        /* the currently running thread cannot have just woken up */
        unsafe {
            assert!((*awakened).get_ptr() != NODE_STATE!(ksCurThread));
            /* round robin threads should not be in the release queue */
            assert!(
                !convert_to_mut_type_ref::<sched_context_t>((*awakened).tcbSchedContext)
                    .is_round_robin()
            );
            #[cfg(feature = "enable_smp")]
            assert!((*awakened).tcbAffinity == cpu_id());
            /* threads HEAD refill should always be >= min_budget */
            assert!(
                convert_to_mut_type_ref::<sched_context_t>((*awakened).tcbSchedContext)
                    .refill_sufficient(0)
            );
            possible_switch_to(&mut *awakened);
        }
    }
}
#[cfg(feature = "kernel_mcs")]
pub fn is_cur_domain_expired() -> bool {
    use sel4_common::sel4_config::NUM_DOMAINS;
    NUM_DOMAINS > 1 && unsafe { ksDomainTime } == 0
}
#[cfg(feature = "kernel_mcs")]
pub fn update_timestamp() {
    use sel4_common::{
        platform::{timer, Timer_func},
        sel4_config::NUM_DOMAINS,
    };

    use crate::sched_context::{max_release_time, min_budget};

    unsafe {
        let prev = NODE_STATE!(ksCurTime);
        SET_NODE_STATE!(ksCurTime = timer.get_current_time());
        assert!(NODE_STATE!(ksCurTime) < max_release_time());
        let consumed = NODE_STATE!(ksCurTime) - prev;
        SET_NODE_STATE!(ksConsumed = NODE_STATE!(ksConsumed) + consumed);
        if NUM_DOMAINS > 1 {
            if consumed + min_budget() >= ksDomainTime {
                ksDomainTime = 0;
            } else {
                ksDomainTime -= consumed;
            }
        }
    }
}
#[cfg(feature = "kernel_mcs")]
pub fn check_domain_time() {
    if unlikely(is_cur_domain_expired()) {
        SET_NODE_STATE!(ksReprogram = true);
        reschedule_required();
    }
}
#[cfg(feature = "kernel_mcs")]
pub fn check_budget() -> bool {
    let current_sched_context = get_current_sc();
    assert!(current_sched_context.refill_ready());
    if likely(current_sched_context.refill_sufficient(NODE_STATE!(ksConsumed))) {
        if unlikely(is_cur_domain_expired()) {
            return false;
        }
        return true;
    }
    charge_budget(NODE_STATE!(ksConsumed), true);
    false
}
#[cfg(feature = "kernel_mcs")]
pub fn check_budget_restart() -> bool {
    assert!(get_currenct_thread().is_runnable());
    let result = check_budget();
    if !result && get_currenct_thread().is_runnable() {
        set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    }
    result
}
#[inline]
pub fn mcs_preemption_point() {
    #[cfg(feature = "kernel_mcs")]
    {
        if get_currenct_thread().is_schedulable() {
            check_budget();
        } else if get_current_sc().scRefillMax != 0 {
            charge_budget(NODE_STATE!(ksConsumed), false);
        } else {
            SET_NODE_STATE!(ksConsumed = 0)
        }
    }
}
#[cfg(feature = "kernel_mcs")]
pub fn set_next_interrupt() {
    use sel4_common::{
        arch::get_timer_precision,
        platform::{timer, Timer_func},
        sel4_config::NUM_DOMAINS,
    };

    unsafe {
        let mut next_interrupt = NODE_STATE!(ksCurTime)
            + (*convert_to_mut_type_ref::<sched_context_t>(get_currenct_thread().tcbSchedContext)
                .refill_head())
            .rAmount;
        if NUM_DOMAINS > 1 {
            next_interrupt = core::cmp::min(next_interrupt, NODE_STATE!(ksCurTime) + ksDomainTime);
        }
        if NODE_STATE!(ksReleaseQueue).head != 0 {
            next_interrupt = core::cmp::min(
                (*convert_to_mut_type_ref::<sched_context_t>(
                    convert_to_mut_type_ref::<tcb_t>(NODE_STATE!(ksReleaseQueue).head)
                        .tcbSchedContext,
                )
                .refill_head())
                .rTime,
                next_interrupt,
            );
        }
        timer.set_deadline(next_interrupt - get_timer_precision());
    }
}
#[cfg(feature = "kernel_mcs")]
pub fn charge_budget(consumed: ticks_t, canTimeoutFault: bool) {
    use crate::{endTimeslice, sched_context::min_budget};

    unsafe {
        if likely(NODE_STATE!(ksCurSC) != NODE_STATE!(ksIdleSC)) {
            let current_sched_context = get_current_sc();
            if current_sched_context.is_round_robin() {
                assert!(current_sched_context.refill_size() == MIN_REFILLS);
                (*current_sched_context.refill_head()).rAmount +=
                    (*current_sched_context.refill_tail()).rAmount;
                (*current_sched_context.refill_tail()).rAmount = 0;
            } else {
                refill_budget_check(consumed);
            }

            assert!((*current_sched_context.refill_head()).rAmount >= min_budget());
            current_sched_context.scConsumed += consumed;
        }
        SET_NODE_STATE!(ksConsumed = 0);
        let thread = get_currenct_thread();
        if likely(thread.is_schedulable()) {
            assert!(thread.tcbSchedContext == NODE_STATE!(ksCurSC));
            endTimeslice(canTimeoutFault);
            reschedule_required();
            SET_NODE_STATE!(ksReprogram = true);
        }
    }
}
#[cfg(feature = "kernel_mcs")]
pub fn commit_time() {
    unsafe {
        let current_sched_context = get_current_sc();
        if likely(
            current_sched_context.scRefillMax != 0 && NODE_STATE!(ksCurSC) != NODE_STATE!(ksIdleSC),
        ) {
            if likely(NODE_STATE!(ksConsumed) > 0) {
                assert!(current_sched_context.refill_sufficient(NODE_STATE!(ksConsumed)));
                assert!(current_sched_context.refill_ready());

                if current_sched_context.is_round_robin() {
                    assert!(current_sched_context.refill_size() == MIN_REFILLS);
                    (*current_sched_context.refill_head()).rAmount -= NODE_STATE!(ksConsumed);
                    (*current_sched_context.refill_tail()).rAmount += NODE_STATE!(ksConsumed);
                } else {
                    refill_budget_check(NODE_STATE!(ksConsumed));
                }
                assert!(current_sched_context.refill_sufficient(0));
                assert!(current_sched_context.refill_ready());
            }
            current_sched_context.scConsumed += NODE_STATE!(ksConsumed);
        }
        SET_NODE_STATE!(ksConsumed = 0);
    }
}
#[cfg(feature = "kernel_mcs")]
pub fn switch_sched_context() {
    use sel4_common::utils::convert_to_option_mut_type_ref;

    let thread = get_currenct_thread();
    if unlikely(NODE_STATE!(ksCurSC) != thread.tcbSchedContext) {
        SET_NODE_STATE!(ksReprogram = true);
        if let Some(sc) = convert_to_option_mut_type_ref::<sched_context_t>(thread.tcbSchedContext)
        {
            if sc.sc_constant_bandwidth() {
                sc.refill_unblock_check();
            }

            assert!(sc.refill_ready());
            assert!(sc.refill_sufficient(0));
        }
    }

    if NODE_STATE!(ksReprogram) {
        commit_time();
    }

    SET_NODE_STATE!(ksCurSC = thread.tcbSchedContext);
}

#[no_mangle]
/// Schedule threads.
pub fn schedule() {
    #[cfg(feature = "kernel_mcs")]
    {
        awaken();
        check_domain_time();
    }
    if NODE_STATE!(ksSchedulerAction) != SCHEDULER_ACTION_RESUME_CURRENT_THREAD {
        let was_runnable: bool;
        let current_tcb = get_currenct_thread();
        if current_tcb.is_schedulable() {
            was_runnable = true;
            current_tcb.sched_enqueue();
        } else {
            was_runnable = false;
        }

        if NODE_STATE!(ksSchedulerAction) == SCHEDULER_ACTION_CHOOSE_NEW_THREAD {
            schedule_choose_new_thread();
        } else {
            // let candidate = ksSchedulerAction as *mut tcb_t;
            let candidate = convert_to_mut_type_ref::<tcb_t>(NODE_STATE!(ksSchedulerAction));
            assert!(candidate.is_schedulable());
            let fastfail = get_currenct_thread().get_ptr() == get_idle_thread().get_ptr()
                || candidate.tcbPriority < get_currenct_thread().tcbPriority;
            if fastfail && !is_highest_prio(unsafe { ksCurDomain }, candidate.tcbPriority) {
                candidate.sched_enqueue();
                // ksSchedulerAction = SCHEDULER_ACTION_CHOOSE_NEW_THREAD;
                SET_NODE_STATE!(ksSchedulerAction = SCHEDULER_ACTION_CHOOSE_NEW_THREAD);
                schedule_choose_new_thread();
            } else if was_runnable && candidate.tcbPriority == get_currenct_thread().tcbPriority {
                candidate.sched_append();
                SET_NODE_STATE!(ksSchedulerAction = SCHEDULER_ACTION_CHOOSE_NEW_THREAD);
                schedule_choose_new_thread();
            } else {
                candidate.switch_to_this();
            }
        }
    }
    SET_NODE_STATE!(ksSchedulerAction = SCHEDULER_ACTION_RESUME_CURRENT_THREAD);
    #[cfg(feature = "enable_smp")]
    unsafe {
        do_mask_reschedule(ksSMP[cpu_id()].ipiReschedulePending);
        ksSMP[cpu_id()].ipiReschedulePending = 0;
    }
    #[cfg(feature = "kernel_mcs")]
    {
        switch_sched_context();
        if NODE_STATE!(ksReprogram) {
            set_next_interrupt();
            SET_NODE_STATE!(ksReprogram = false);
        }
    }
}

#[inline]
/// Schedule the given tcb.
pub fn schedule_tcb(tcb_ref: &tcb_t) {
    if tcb_ref.get_ptr() == NODE_STATE!(ksCurThread)
        && NODE_STATE!(ksSchedulerAction) == SCHEDULER_ACTION_RESUME_CURRENT_THREAD
        && !tcb_ref.is_schedulable()
    {
        reschedule_required();
    }
}

#[cfg(feature = "enable_smp")]
#[inline]
/// Schedule the given tcb when current tcb is not in the same domain or not in the same cpu or current action is not to resume the current thread.
pub fn possible_switch_to(target: &mut tcb_t) {
    #[cfg(not(feature = "kernel_mcs"))]
    {
        if unsafe { ksCurDomain != target.domain || target.tcbAffinity != cpu_id() } {
            target.sched_enqueue();
        } else if NODE_STATE!(ksSchedulerAction) != SCHEDULER_ACTION_RESUME_CURRENT_THREAD {
            reschedule_required();
            target.sched_enqueue();
        } else {
            SET_NODE_STATE!(ksSchedulerAction = target.get_ptr());
        }
    }
    #[cfg(feature = "kernel_mcs")]
    {
        if target.tcbSchedContext != 0 && target.tcbState.get_tcbInReleaseQueue() == 0 {
            if unsafe { ksCurDomain != target.domain || target.tcbAffinity != cpu_id() } {
                target.sched_enqueue();
            } else if NODE_STATE!(ksSchedulerAction) != SCHEDULER_ACTION_RESUME_CURRENT_THREAD {
                reschedule_required();
                target.sched_enqueue();
            } else {
                SET_NODE_STATE!(ksSchedulerAction = target.get_ptr());
            }
        }
    }
}

#[cfg(not(feature = "enable_smp"))]
#[inline]
/// Schedule the given tcb when current tcb is not in the same domain or current action is not to resume the current thread.
pub fn possible_switch_to(target: &mut tcb_t) {
    #[cfg(not(feature = "kernel_mcs"))]
    {
        if unsafe { ksCurDomain != target.domain } {
            target.sched_enqueue();
        } else if NODE_STATE!(ksSchedulerAction) != SCHEDULER_ACTION_RESUME_CURRENT_THREAD {
            reschedule_required();
            target.sched_enqueue();
        } else {
            SET_NODE_STATE!(ksSchedulerAction = target.get_ptr());
        }
    }
    #[cfg(feature = "kernel_mcs")]
    {
        if target.tcbSchedContext != 0 && target.tcbState.get_tcbInReleaseQueue() == 0 {
            if unsafe { ksCurDomain != target.domain } {
                target.sched_enqueue();
            } else if NODE_STATE!(ksSchedulerAction) != SCHEDULER_ACTION_RESUME_CURRENT_THREAD {
                reschedule_required();
                target.sched_enqueue();
            } else {
                SET_NODE_STATE!(ksSchedulerAction = target.get_ptr());
            }
        }
    }
}

#[no_mangle]
/// Schedule current thread if time slice is expired.
pub fn timer_tick() {
    let current = get_currenct_thread();
    // if hart_id() == 0 {
    //     debug!("timer tick current: {:#x}", current.get_ptr());
    // }

    if likely(current.get_state() == ThreadState::ThreadStateRunning) {
        if current.tcbTimeSlice > 1 {
            // if hart_id() == 0 {
            //     debug!("tcbTimeSlice : {}", current.tcbTimeSlice);
            // }
            current.tcbTimeSlice -= 1;
        } else {
            // if hart_id() == 0 {
            //     debug!("switch");
            // }

            current.tcbTimeSlice = CONFIG_TIME_SLICE;
            current.sched_append();
            reschedule_required();
        }
    }
}

#[no_mangle]
/// Activate the current thread.
pub fn activateThread() {
    let thread = get_currenct_thread();
    // debug!("current: {:#x}", thread.get_ptr());
    #[cfg(feature = "kernel_mcs")]
    {
        // TODO: MCS
        // #ifdef CONFIG_KERNEL_MCS
        //     if (unlikely(NODE_STATE(ksCurThread)->tcbYieldTo))
        //     {
        //         schedContext_completeYieldTo(NODE_STATE(ksCurThread));
        //         assert(thread_state_get_tsType(NODE_STATE(ksCurThread)->tcbState) == ThreadState_Running);
        //     }
        // #endif
        if unlikely(thread.tcbYieldTo != 0) {
            thread.schedContext_completeYieldTo();
            assert!(thread.tcbState.get_tsType() == ThreadState::ThreadStateRunning as u64);
        }
    }
    match thread.get_state() {
        ThreadState::ThreadStateRunning => {
            return;
        }
        ThreadState::ThreadStateRestart => {
            let pc = thread.tcbArch.get_register(ArchReg::FAULT_IP);
            // setNextPC(thread, pc);
            // sel4_common::println!("restart pc is {:x}",pc);
            thread.tcbArch.set_register(ArchReg::NEXT_IP, pc);
            // set_thread_state(thread, ThreadStateRunning);
            set_thread_state(thread, ThreadState::ThreadStateRunning);
        }
        // 诡异的语法...
        ThreadState::ThreadStateIdleThreadState => return {},
        #[cfg(not(feature = "enable_smp"))]
        _ => panic!(
            "current thread is blocked , state id :{}",
            thread.get_state() as usize
        ),
        #[cfg(feature = "enable_smp")]
        _ => panic!(
            "current thread is blocked , state id :{}, cpu: {}",
            thread.get_state() as usize,
            thread.tcbAffinity
        ),
    }
}
#[cfg(feature = "kernel_mcs")]
pub fn configure_sched_context(tcb: &mut tcb_t, sc_pptr: &mut sched_context_t, timeslice: ticks_t) {
    tcb.tcbSchedContext = sc_pptr.get_ptr();
    sc_pptr.refill_new(MIN_REFILLS, timeslice, 0);
    sc_pptr.scTcb = tcb.get_ptr();
}

#[cfg(not(feature = "enable_smp"))]
/// Create the idle thread.
pub fn create_idle_thread() {
    unsafe {
        let pptr = &mut ksIdleThreadTCB.data[0][0] as *mut u8 as *mut usize;
        // let pptr = ksIdleThreadTCB as usize as *mut usize;
        ksIdleThread = ptr_to_usize_add(pptr, TCB_OFFSET);
        // let tcb = convert_to_mut_type_ref::<tcb_t>(ksIdleThread as usize);
        let tcb = get_idle_thread();
        // Arch_configureIdleThread(tcb.tcbArch);
        tcb.tcbArch.config_idle_thread(idle_thread as usize, 0);
        set_thread_state(tcb, ThreadState::ThreadStateIdleThreadState);
        #[cfg(feature = "kernel_mcs")]
        {
            tcb.tcbYieldTo = 0;
            configure_sched_context(
                convert_to_mut_type_ref::<tcb_t>(ksIdleThread),
                convert_to_mut_type_ref::<sched_context_t>(
                    &mut ksIdleThreadSC.data[0] as *mut u8 as usize,
                ),
                us_to_ticks(CONFIG_BOOT_THREAD_TIME_SLICE * US_IN_MS),
            );
            SET_NODE_STATE!(ksIdleSC = &mut ksIdleThreadSC.data[0] as *mut u8 as usize)
        }
    }
}

#[cfg(feature = "enable_smp")]
pub fn create_idle_thread() {
    unsafe {
        for i in 0..CONFIG_MAX_NUM_NODES {
            // debug!("ksIdleThread: {:#x}", ksSMP[i].ksIdleThread);
            let pptr = &mut ksIdleThreadTCB.data[i] as *mut u8 as *mut usize;
            ksSMP[i].ksIdleThread = ptr_to_usize_add(pptr, TCB_OFFSET);
            let tcb = convert_to_mut_type_ref::<tcb_t>(ksSMP[i].ksIdleThread);
            tcb.tcbArch.config_idle_thread(idle_thread as usize, i);
            set_thread_state(tcb, ThreadState::ThreadStateIdleThreadState);
            tcb.tcbAffinity = i;
            #[cfg(feature = "kernel_mcs")]
            {
                tcb.tcbYieldTo = 0;
                configure_sched_context(
                    convert_to_mut_type_ref::<tcb_t>(ksSMP[i].ksIdleThread),
                    convert_to_mut_type_ref::<sched_context_t>(
                        &mut ksIdleThreadSC.data[i] as *mut u8 as usize,
                    ),
                    us_to_ticks(CONFIG_BOOT_THREAD_TIME_SLICE * US_IN_MS),
                );
                SET_NODE_STATE!(ksIdleSC = &mut ksIdleThreadSC.data[i] as *mut u8 as usize);
            }
        }
    }
}

pub fn idle_thread() {
    unsafe {
        loop {
            // debug!("hello idle_thread");
            asm!("wfi");
        }
    }
}
