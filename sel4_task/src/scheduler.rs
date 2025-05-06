//! This module contains the implementation of the scheduler for the sel4_task crate.
//!
//! It includes functions and data structures related to task scheduling and thread management.
//! The scheduler supports Symmetric Multiprocessing (SMP) and provides functionality for choosing
//! new threads to run, managing ready queues, and handling domain scheduling.
//!
#[cfg(feature = "ENABLE_SMP")]
use crate::deps::doMaskReschedule;
use core::arch::asm;
use core::intrinsics::{likely, unlikely};
use sel4_common::arch::ArchReg;
#[cfg(feature = "ENABLE_SMP")]
use sel4_common::ffi::kernel_stack_alloc;
#[cfg(feature = "ENABLE_SMP")]
use sel4_common::sel4_config::{seL4_TCBBits, CONFIG_MAX_NUM_NODES};
use sel4_common::sel4_config::{
    wordBits, wordRadix, CONFIG_NUM_DOMAINS, CONFIG_NUM_PRIORITIES, CONFIG_TIME_SLICE,
    L2_BITMAP_SIZE, NUM_READY_QUEUES, TCB_OFFSET,
};
use sel4_common::utils::{
    convert_to_mut_type_ref, convert_to_mut_type_ref_unsafe, cpu_id, ptr_to_usize_add,
};
use sel4_common::{BIT, MASK};

use crate::deps::ksIdleThreadTCB;
#[cfg(feature = "KERNEL_MCS")]
use crate::sched_context::{sched_context_t, MIN_REFILLS};
use crate::tcb::{set_thread_state, tcb_t};
use crate::tcb_queue::tcb_queue_t;
use crate::thread_state::ThreadState;
#[cfg(feature = "KERNEL_MCS")]
use crate::{deps::ksIdleThreadSC, sched_context::refill_budget_check, tcb_Release_Dequeue};
#[cfg(feature = "KERNEL_MCS")]
use sel4_common::{
    arch::usToTicks,
    platform::time_def::{ticks_t, time_t, US_IN_MS},
    sel4_config::CONFIG_BOOT_THREAD_TIME_SLICE,
};
#[cfg(target_arch = "aarch64")]
use sel4_vspace::{
    get_arm_global_user_vspace_base, kpptr_to_paddr, setCurrentUserVSpaceRoot, ttbr_new,
};

#[cfg(feature = "ENABLE_SMP")]
#[derive(Debug, Copy, Clone)]
/// Struct representing the SMP (Symmetric Multiprocessing) state data.
pub struct SmpStateData {
    /// Number of pending IPI (Inter-Processor Interrupt) reschedule requests.
    pub ipiReschedulePending: usize,
    /// Array of ready queues for each domain and priority level.
    pub ksReadyQueues: [tcb_queue_t; CONFIG_NUM_DOMAINS * CONFIG_NUM_PRIORITIES],
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
    /// Number of debug TCBs (Thread Control Blocks).
    pub fpu1: usize,
    // TODO: Cache Line 对齐
    pub fpu2: usize,
    // TODO: Cache Line 对齐
    pub ksDebugTCBs: usize,
    // TODO: Cache Line 对齐
}

#[cfg(feature = "ENABLE_SMP")]
#[no_mangle]
pub static mut ksSMP: [SmpStateData; CONFIG_MAX_NUM_NODES] = [SmpStateData {
    ipiReschedulePending: 0,
    ksReadyQueues: [tcb_queue_t { head: 0, tail: 0 }; CONFIG_NUM_DOMAINS * CONFIG_NUM_PRIORITIES],
    ksReadyQueuesL1Bitmap: [0; CONFIG_NUM_DOMAINS],
    ksReadyQueuesL2Bitmap: [[0; L2_BITMAP_SIZE]; CONFIG_NUM_DOMAINS],
    ksCurThread: 0,
    ksIdleThread: 0,
    ksSchedulerAction: 1,
    fpu1: 0,
    fpu2: 0,
    ksDebugTCBs: 0,
}; CONFIG_MAX_NUM_NODES];

#[repr(C)]
#[derive(Debug, PartialEq, Clone, Copy)]
/// Struct representing a domain schedule.
pub struct dschedule_t {
    /// Domain ID.
    pub domain: usize,
    /// Length of the domain schedule.
    pub length: usize,
}

pub const SchedulerAction_ResumeCurrentThread: usize = 0;
pub const SchedulerAction_ChooseNewThread: usize = 1;
pub const ksDomScheduleLength: usize = 1;

pub const seL4_SchedContext_NoFlag: usize = 0;
pub const seL4_SchedContext_Sporadic: usize = 1;
#[no_mangle]
pub static mut ksDomainTime: usize = 0;

#[no_mangle]
pub static mut ksCurDomain: usize = 0;

#[no_mangle]
pub static mut ksDomScheduleIdx: usize = 0;

#[no_mangle]
pub static mut ksCurThread: usize = 0;

#[no_mangle]
pub static mut ksIdleThread: usize = 0;

#[no_mangle]
pub static mut ksSchedulerAction: usize = 1;

#[no_mangle]
#[cfg(feature = "KERNEL_MCS")]
pub static mut ksReleaseQueue: tcb_queue_t = tcb_queue_t { head: 0, tail: 0 };

#[no_mangle]
#[cfg(feature = "KERNEL_MCS")]
pub static mut ksCurSC: usize = 0;

#[no_mangle]
#[cfg(feature = "KERNEL_MCS")]
pub static mut ksConsumed: time_t = 0;

#[no_mangle]
#[cfg(feature = "KERNEL_MCS")]
pub static mut ksCurTime: time_t = 0;

#[no_mangle]
#[cfg(feature = "KERNEL_MCS")]
pub static mut ksReprogram: bool = false;

#[no_mangle]
#[cfg(feature = "KERNEL_MCS")]
pub static mut ksIdleSC: usize = 0;

#[no_mangle]
pub static mut ksReadyQueues: [tcb_queue_t; NUM_READY_QUEUES] =
    [tcb_queue_t { head: 0, tail: 0 }; NUM_READY_QUEUES];

#[no_mangle]
pub static mut ksReadyQueuesL2Bitmap: [[usize; L2_BITMAP_SIZE]; CONFIG_NUM_DOMAINS] =
    [[0; L2_BITMAP_SIZE]; CONFIG_NUM_DOMAINS];

#[no_mangle]
pub static mut ksReadyQueuesL1Bitmap: [usize; CONFIG_NUM_DOMAINS] = [0; CONFIG_NUM_DOMAINS];

#[no_mangle]
#[link_section = ".boot.bss"]
pub static mut ksWorkUnitsCompleted: usize = 0;

// #[link_section = ".boot.bss"]
pub static mut ksDomSchedule: [dschedule_t; ksDomScheduleLength] = [dschedule_t {
    domain: 0,
    length: 60,
}; ksDomScheduleLength];

#[allow(non_camel_case_types)]
pub type prio_t = usize;

#[inline]
/// Get the idle thread, and returns a mutable tcb reference to the idle thread.
pub fn get_idle_thread() -> &'static mut tcb_t {
    unsafe {
        #[cfg(feature = "ENABLE_SMP")]
        {
            convert_to_mut_type_ref::<tcb_t>(ksSMP[cpu_id()].ksIdleThread)
        }
        #[cfg(not(feature = "ENABLE_SMP"))]
        {
            convert_to_mut_type_ref::<tcb_t>(ksIdleThread)
        }
    }
}

#[inline]
/// Get the action to be taken by ks scheduler.
pub fn get_ks_scheduler_action() -> usize {
    unsafe {
        #[cfg(feature = "ENABLE_SMP")]
        {
            ksSMP[cpu_id()].ksSchedulerAction
        }
        #[cfg(not(feature = "ENABLE_SMP"))]
        {
            ksSchedulerAction
        }
    }
}

#[inline]
/// Set the action to be taken by ks scheduler.
pub fn set_ks_scheduler_action(action: usize) {
    // if hart_id() == 0 {
    //     debug!("set_ks_scheduler_action: {}", action);
    // }
    unsafe {
        #[cfg(feature = "ENABLE_SMP")]
        {
            ksSMP[cpu_id()].ksSchedulerAction = action;
        }
        #[cfg(not(feature = "ENABLE_SMP"))]
        {
            ksSchedulerAction = action
        }
    }
}

#[inline]
/// Get the current thread, and returns a mutable tcb reference to the current thread.
/// FIXME: fix the name of this function, get_current_thread
pub fn get_currenct_thread() -> &'static mut tcb_t {
    unsafe {
        #[cfg(feature = "ENABLE_SMP")]
        {
            convert_to_mut_type_ref::<tcb_t>(ksSMP[cpu_id()].ksCurThread)
        }
        #[cfg(not(feature = "ENABLE_SMP"))]
        {
            convert_to_mut_type_ref::<tcb_t>(ksCurThread)
        }
    }
}

#[inline]
/// Get the current thread, and returns a mutable tcb reference to the current thread unsafely.
pub fn get_currenct_thread_unsafe() -> &'static mut tcb_t {
    unsafe {
        #[cfg(feature = "ENABLE_SMP")]
        {
            convert_to_mut_type_ref_unsafe::<tcb_t>(ksSMP[cpu_id()].ksCurThread)
        }
        #[cfg(not(feature = "ENABLE_SMP"))]
        {
            convert_to_mut_type_ref_unsafe::<tcb_t>(ksCurThread)
        }
    }
}

#[inline]
/// Set the action to be taken by current scheduler.
pub fn set_current_scheduler_action(action: usize) {
    unsafe {
        #[cfg(feature = "ENABLE_SMP")]
        {
            ksSMP[cpu_id()].ksSchedulerAction = action;
        }
        #[cfg(not(feature = "ENABLE_SMP"))]
        {
            ksSchedulerAction = action;
        }
    }
}

#[inline]
/// Set the current thread.
pub fn set_current_thread(thread: &tcb_t) {
    unsafe {
        #[cfg(feature = "ENABLE_SMP")]
        {
            ksSMP[cpu_id()].ksCurThread = thread.get_ptr();
        }
        #[cfg(not(feature = "ENABLE_SMP"))]
        {
            ksCurThread = thread.get_ptr()
        }
    }
}

#[inline]
/// Get the current domain.
pub fn get_current_domain() -> usize {
    unsafe { ksCurDomain }
}

#[inline]
#[cfg(feature = "KERNEL_MCS")]
pub fn get_current_sc() -> &'static mut sched_context_t {
    unsafe {
        #[cfg(feature = "ENABLE_SMP")]
        {
            //TODO: SMP
        }
        #[cfg(not(feature = "ENABLE_SMP"))]
        {
            convert_to_mut_type_ref_unsafe::<sched_context_t>(ksCurSC)
        }
    }
}

#[inline]
/// Get the index of the ready queue for the given domain and priority level.
pub fn ready_queues_index(dom: usize, prio: usize) -> usize {
    dom * CONFIG_NUM_PRIORITIES + prio
}

#[inline]
/// Get the L1 index for the given priority level.
fn prio_to_l1index(prio: usize) -> usize {
    prio >> wordRadix
}

#[inline]
/// Get the priority level for the given L1 index.
fn l1index_to_prio(l1index: usize) -> usize {
    l1index << wordRadix
}

#[inline]
/// Invert the L1 index.
fn invert_l1index(l1index: usize) -> usize {
    let inverted = L2_BITMAP_SIZE - 1 - l1index;
    inverted
}

#[cfg(not(feature = "ENABLE_SMP"))]
#[inline]
/// Get the highest priority level for the given domain in single-core mode.
fn getHighestPrio(dom: usize) -> prio_t {
    unsafe {
        let l1index = wordBits - 1 - ksReadyQueuesL1Bitmap[dom].leading_zeros() as usize;
        let l1index_inverted = invert_l1index(l1index);
        let l2index =
            wordBits - 1 - ksReadyQueuesL2Bitmap[dom][l1index_inverted].leading_zeros() as usize;
        l1index_to_prio(l1index) | l2index
    }
}

#[cfg(feature = "ENABLE_SMP")]
#[inline]
/// Get the highest priority level for the given domain on the current CPU in multi-core mode.
fn getHighestPrio(dom: usize) -> prio_t {
    unsafe {
        let l1index =
            wordBits - 1 - ksSMP[cpu_id()].ksReadyQueuesL1Bitmap[dom].leading_zeros() as usize;
        let l1index_inverted = invert_l1index(l1index);
        let l2index = wordBits
            - 1
            - (ksSMP[cpu_id()].ksReadyQueuesL2Bitmap[dom])[l1index_inverted].leading_zeros()
                as usize;
        l1index_to_prio(l1index) | l2index
    }
}

#[inline]
/// Check if the given priority level is the highest priority level for the given domain.
pub fn isHighestPrio(dom: usize, prio: prio_t) -> bool {
    #[cfg(feature = "ENABLE_SMP")]
    {
        unsafe { ksSMP[cpu_id()].ksReadyQueuesL1Bitmap[dom] == 0 || prio >= getHighestPrio(dom) }
    }
    #[cfg(not(feature = "ENABLE_SMP"))]
    {
        unsafe { ksReadyQueuesL1Bitmap[dom] == 0 || prio >= getHighestPrio(dom) }
    }
}

#[inline]
/// Add the given priority level to the ready queue bitmap.
pub fn addToBitmap(_cpu: usize, dom: usize, prio: usize) {
    unsafe {
        let l1index = prio_to_l1index(prio);
        let l1index_inverted = invert_l1index(l1index);
        #[cfg(feature = "ENABLE_SMP")]
        {
            ksSMP[_cpu].ksReadyQueuesL1Bitmap[dom] |= BIT!(l1index);
            ksSMP[_cpu].ksReadyQueuesL2Bitmap[dom][l1index_inverted] |=
                BIT!(prio & MASK!(wordRadix));
        }
        #[cfg(not(feature = "ENABLE_SMP"))]
        {
            ksReadyQueuesL1Bitmap[dom] |= BIT!(l1index);
            ksReadyQueuesL2Bitmap[dom][l1index_inverted] |= BIT!(prio & MASK!(wordRadix));
        }
    }
}

#[inline]
/// Remove the given priority level from the ready queue bitmap.
pub fn removeFromBitmap(_cpu: usize, dom: usize, prio: usize) {
    unsafe {
        let l1index = prio_to_l1index(prio);
        let l1index_inverted = invert_l1index(l1index);
        #[cfg(feature = "ENABLE_SMP")]
        {
            ksSMP[_cpu].ksReadyQueuesL2Bitmap[dom][l1index_inverted] &=
                !BIT!(prio & MASK!(wordRadix));
            if unlikely(ksSMP[_cpu].ksReadyQueuesL2Bitmap[dom][l1index_inverted] == 0) {
                ksSMP[_cpu].ksReadyQueuesL1Bitmap[dom] &= !(BIT!((l1index)));
            }
        }
        #[cfg(not(feature = "ENABLE_SMP"))]
        {
            ksReadyQueuesL2Bitmap[dom][l1index_inverted] &= !BIT!(prio & MASK!(wordRadix));
            if unlikely(ksReadyQueuesL2Bitmap[dom][l1index_inverted] == 0) {
                ksReadyQueuesL1Bitmap[dom] &= !(BIT!((l1index)));
            }
        }
    }
}

fn nextDomain() {
    unsafe {
        ksDomScheduleIdx += 1;
        if ksDomScheduleIdx >= ksDomScheduleLength {
            ksDomScheduleIdx = 0;
        }
        #[cfg(feature = "KERNEL_MCS")]
        {
            ksReprogram = true;
        }
        ksWorkUnitsCompleted = 0;
        ksCurDomain = ksDomSchedule[ksDomScheduleIdx].domain;
        #[cfg(feature = "KERNEL_MCS")]
        {
            ksDomainTime = usToTicks(ksDomSchedule[ksDomScheduleIdx].length * US_IN_MS);
        }
        #[cfg(not(feature = "KERNEL_MCS"))]
        {
            ksDomainTime = ksDomSchedule[ksDomScheduleIdx].length;
        }
        //FIXME ksWorkUnits not used;
        // ksWorkUnits
    }
}

fn scheduleChooseNewThread() {
    // if hart_id() == 0 {
    //     debug!("scheduleChooseNewThread");
    // }

    unsafe {
        if ksDomainTime == 0 {
            nextDomain();
        }
    }
    chooseThread();
}

fn chooseThread() {
    unsafe {
        let dom = 0;
        let ks_l1_bit = {
            #[cfg(feature = "ENABLE_SMP")]
            {
                ksSMP[cpu_id()].ksReadyQueuesL1Bitmap[dom]
            }
            #[cfg(not(feature = "ENABLE_SMP"))]
            {
                ksReadyQueuesL1Bitmap[dom]
            }
        };
        if likely(ks_l1_bit != 0) {
            let prio = getHighestPrio(dom);
            let thread = {
                #[cfg(feature = "ENABLE_SMP")]
                {
                    ksSMP[cpu_id()].ksReadyQueues[ready_queues_index(dom, prio)].head
                }
                #[cfg(not(feature = "ENABLE_SMP"))]
                {
                    ksReadyQueues[ready_queues_index(dom, prio)].head
                }
            };
            assert_ne!(thread, 0);
            assert!(convert_to_mut_type_ref::<tcb_t>(thread).is_schedulable());
            #[cfg(feature = "KERNEL_MCS")]
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
                setCurrentUserVSpaceRoot(ttbr_new(
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
#[cfg(not(feature = "KERNEL_MCS"))]
/// Reschedule threads, and enqueue the current thread if current ks scheduler action is not to resume the current thread and choose new thread.
pub fn rescheduleRequired() {
    if get_ks_scheduler_action() != SchedulerAction_ResumeCurrentThread
        && get_ks_scheduler_action() != SchedulerAction_ChooseNewThread
    {
        convert_to_mut_type_ref::<tcb_t>(get_ks_scheduler_action()).sched_enqueue();
    }
    // ksSchedulerAction = SchedulerAction_ChooseNewThread;
    set_ks_scheduler_action(SchedulerAction_ChooseNewThread);
}
#[no_mangle]
#[cfg(feature = "KERNEL_MCS")]
/// Reschedule threads, and enqueue the current thread if current ks scheduler action is not to resume the current thread and choose new thread.
pub fn rescheduleRequired() {
    let action = get_ks_scheduler_action();
    if action != SchedulerAction_ResumeCurrentThread && action != SchedulerAction_ChooseNewThread {
        let action_tcb = convert_to_mut_type_ref::<tcb_t>(action);
        if action_tcb.is_schedulable() {
            let action_sched_context =
                convert_to_mut_type_ref::<sched_context_t>(action_tcb.tcbSchedContext);
            assert!(action_sched_context.refill_sufficient(0));
            assert!(action_sched_context.refill_ready());
            action_tcb.sched_enqueue();
        }
    }
    set_ks_scheduler_action(SchedulerAction_ChooseNewThread);
}
#[cfg(feature = "KERNEL_MCS")]
pub fn awaken() {
    while unsafe {
        unlikely(
            ksReleaseQueue.head != 0
                && convert_to_mut_type_ref::<sched_context_t>(
                    convert_to_mut_type_ref::<tcb_t>(ksReleaseQueue.head).tcbSchedContext,
                )
                .refill_ready(),
        )
    } {
        let awakened = tcb_Release_Dequeue();
        /* the currently running thread cannot have just woken up */
        unsafe {
            assert!((*awakened).get_ptr() != ksCurThread);
            /* round robin threads should not be in the release queue */
            assert!(
                !convert_to_mut_type_ref::<sched_context_t>((*awakened).tcbSchedContext)
                    .is_round_robin()
            );
            /* threads HEAD refill should always be >= MIN_BUDGET */
            assert!(
                convert_to_mut_type_ref::<sched_context_t>((*awakened).tcbSchedContext)
                    .refill_sufficient(0)
            );
            possible_switch_to(&mut *awakened);
        }
    }
}
#[cfg(feature = "KERNEL_MCS")]
pub fn isCurDomainExpired() -> bool {
    use sel4_common::sel4_config::numDomains;
    numDomains > 1 && unsafe { ksDomainTime } == 0
}
#[cfg(feature = "KERNEL_MCS")]
pub fn updateTimestamp() {
    use sel4_common::{
        platform::{timer, Timer_func},
        sel4_config::numDomains,
    };

    use crate::sched_context::{MAX_RELEASE_TIME, MIN_BUDGET};

    unsafe {
        let prev = ksCurTime;
        ksCurTime = timer.getCurrentTime();
        assert!(ksCurTime < MAX_RELEASE_TIME());
        let consumed = ksCurTime - prev;
        ksConsumed += consumed;
        if numDomains > 1 {
            if consumed + MIN_BUDGET() >= ksDomainTime {
                ksDomainTime = 0;
            } else {
                ksDomainTime -= consumed;
            }
        }
    }
}
#[cfg(feature = "KERNEL_MCS")]
pub fn checkDomainTime() {
    if unlikely(isCurDomainExpired()) {
        unsafe { ksReprogram = true };
        rescheduleRequired();
    }
}
#[cfg(feature = "KERNEL_MCS")]
pub fn checkBudget() -> bool {
    unsafe {
        let current_sched_context = get_current_sc();
        assert!(current_sched_context.refill_ready());
        if likely(current_sched_context.refill_sufficient(ksConsumed)) {
            if unlikely(isCurDomainExpired()) {
                return false;
            }
            return true;
        }
        chargeBudget(ksConsumed, true);
    }
    false
}
#[cfg(feature = "KERNEL_MCS")]
pub fn checkBudgetRestart() -> bool {
    assert!(get_currenct_thread().is_runnable());
    let result = checkBudget();
    if !result && get_currenct_thread().is_runnable() {
        set_thread_state(get_currenct_thread(), ThreadState::ThreadStateRestart);
    }
    result
}
#[inline]
pub fn mcs_preemption_point() {
    #[cfg(feature = "KERNEL_MCS")]
    unsafe {
        if get_currenct_thread().is_schedulable() {
            checkBudget();
        } else if get_current_sc().scRefillMax != 0 {
            chargeBudget(ksConsumed, false);
        } else {
            ksConsumed = 0;
        }
    }
}
#[cfg(feature = "KERNEL_MCS")]
pub fn setNextInterrupt() {
    use sel4_common::{
        arch::getTimerPrecision,
        platform::{timer, Timer_func},
        sel4_config::numDomains,
    };

    unsafe {
        let mut next_interrupt = ksCurTime
            + (*convert_to_mut_type_ref::<sched_context_t>(get_currenct_thread().tcbSchedContext)
                .refill_head())
            .rAmount;
        if numDomains > 1 {
            next_interrupt = core::cmp::min(next_interrupt, ksCurTime + ksDomainTime);
        }
        if ksReleaseQueue.head != 0 {
            next_interrupt = core::cmp::min(
                (*convert_to_mut_type_ref::<sched_context_t>(
                    convert_to_mut_type_ref::<tcb_t>(ksReleaseQueue.head).tcbSchedContext,
                )
                .refill_head())
                .rTime,
                next_interrupt,
            );
        }
        timer.setDeadline(next_interrupt - getTimerPrecision());
    }
}
#[cfg(feature = "KERNEL_MCS")]
pub fn chargeBudget(consumed: ticks_t, canTimeoutFault: bool) {
    use crate::{endTimeslice, sched_context::MIN_BUDGET};

    unsafe {
        if likely(ksCurSC != ksIdleSC) {
            let current_sched_context = get_current_sc();
            if current_sched_context.is_round_robin() {
                assert!(current_sched_context.refill_size() == MIN_REFILLS);
                (*current_sched_context.refill_head()).rAmount +=
                    (*current_sched_context.refill_tail()).rAmount;
                (*current_sched_context.refill_tail()).rAmount = 0;
            } else {
                refill_budget_check(consumed);
            }

            assert!((*current_sched_context.refill_head()).rAmount >= MIN_BUDGET());
            current_sched_context.scConsumed += consumed;
        }
        ksConsumed = 0;
        let thread = get_currenct_thread();
        if likely(thread.is_schedulable()) {
            assert!(thread.tcbSchedContext == ksCurSC);
            endTimeslice(canTimeoutFault);
            rescheduleRequired();
            ksReprogram = true;
        }
    }
}
#[cfg(feature = "KERNEL_MCS")]
pub fn commitTime() {
    unsafe {
        let current_sched_context = get_current_sc();
        if likely(current_sched_context.scRefillMax != 0 && ksCurSC != ksIdleSC) {
            if likely(ksConsumed > 0) {
                assert!(current_sched_context.refill_sufficient(ksConsumed));
                assert!(current_sched_context.refill_ready());

                if current_sched_context.is_round_robin() {
                    assert!(current_sched_context.refill_size() == MIN_REFILLS);
                    (*current_sched_context.refill_head()).rAmount -= ksConsumed;
                    (*current_sched_context.refill_tail()).rAmount += ksConsumed;
                } else {
                    refill_budget_check(ksConsumed);
                }
                assert!(current_sched_context.refill_sufficient(0));
                assert!(current_sched_context.refill_ready());
            }
            current_sched_context.scConsumed += ksConsumed;
        }
        ksConsumed = 0;
    }
}
#[cfg(feature = "KERNEL_MCS")]
pub fn switch_sched_context() {
    use sel4_common::utils::convert_to_option_mut_type_ref;

    let thread = get_currenct_thread();
    unsafe {
        if unlikely(ksCurSC != thread.tcbSchedContext) {
            ksReprogram = true;
            if let Some(sc) =
                convert_to_option_mut_type_ref::<sched_context_t>(thread.tcbSchedContext)
            {
                if sc.sc_constant_bandwidth() {
                    sc.refill_unblock_check();
                }

                assert!(sc.refill_ready());
                assert!(sc.refill_sufficient(0));
            }
        }

        if ksReprogram {
            commitTime();
        }

        ksCurSC = thread.tcbSchedContext;
    }
}

#[no_mangle]
/// Schedule threads.
pub fn schedule() {
    #[cfg(feature = "KERNEL_MCS")]
    {
        awaken();
        checkDomainTime();
    }
    if get_ks_scheduler_action() != SchedulerAction_ResumeCurrentThread {
        let was_runnable: bool;
        let current_tcb = get_currenct_thread();
        if current_tcb.is_schedulable() {
            was_runnable = true;
            current_tcb.sched_enqueue();
        } else {
            was_runnable = false;
        }

        if get_ks_scheduler_action() == SchedulerAction_ChooseNewThread {
            scheduleChooseNewThread();
        } else {
            // let candidate = ksSchedulerAction as *mut tcb_t;
            let candidate = convert_to_mut_type_ref::<tcb_t>(get_ks_scheduler_action());
            assert!(candidate.is_schedulable());
            let fastfail = get_currenct_thread().get_ptr() == get_idle_thread().get_ptr()
                || candidate.tcbPriority < get_currenct_thread().tcbPriority;
            if fastfail && !isHighestPrio(unsafe { ksCurDomain }, candidate.tcbPriority) {
                candidate.sched_enqueue();
                // ksSchedulerAction = SchedulerAction_ChooseNewThread;
                set_ks_scheduler_action(SchedulerAction_ChooseNewThread);
                scheduleChooseNewThread();
            } else if was_runnable && candidate.tcbPriority == get_currenct_thread().tcbPriority {
                candidate.sched_append();
                set_ks_scheduler_action(SchedulerAction_ChooseNewThread);
                scheduleChooseNewThread();
            } else {
                candidate.switch_to_this();
            }
        }
    }
    set_ks_scheduler_action(SchedulerAction_ResumeCurrentThread);
    #[cfg(feature = "ENABLE_SMP")]
    unsafe {
        doMaskReschedule(ksSMP[cpu_id()].ipiReschedulePending);
        ksSMP[cpu_id()].ipiReschedulePending = 0;
    }
    #[cfg(feature = "KERNEL_MCS")]
    {
        switch_sched_context();
        if unsafe { ksReprogram } {
            setNextInterrupt();
            unsafe { ksReprogram = false };
        }
    }
}

#[inline]
/// Schedule the given tcb.
pub fn schedule_tcb(tcb_ref: &tcb_t) {
    if tcb_ref.get_ptr() == get_currenct_thread_unsafe().get_ptr()
        && get_ks_scheduler_action() == SchedulerAction_ResumeCurrentThread
        && !tcb_ref.is_schedulable()
    {
        rescheduleRequired();
    }
}

#[cfg(feature = "ENABLE_SMP")]
#[inline]
/// Schedule the given tcb when current tcb is not in the same domain or not in the same cpu or current action is not to resume the current thread.
pub fn possible_switch_to(target: &mut tcb_t) {
    if unsafe { ksCurDomain != target.domain || target.tcbAffinity != cpu_id() } {
        target.sched_enqueue();
    } else if get_ks_scheduler_action() != SchedulerAction_ResumeCurrentThread {
        rescheduleRequired();
        target.sched_enqueue();
    } else {
        set_ks_scheduler_action(target.get_ptr());
    }
}

#[cfg(not(feature = "ENABLE_SMP"))]
#[inline]
/// Schedule the given tcb when current tcb is not in the same domain or current action is not to resume the current thread.
pub fn possible_switch_to(target: &mut tcb_t) {
    #[cfg(not(feature = "KERNEL_MCS"))]
    {
        if unsafe { ksCurDomain != target.domain } {
            target.sched_enqueue();
        } else if get_ks_scheduler_action() != SchedulerAction_ResumeCurrentThread {
            rescheduleRequired();
            target.sched_enqueue();
        } else {
            set_ks_scheduler_action(target.get_ptr());
        }
    }
    #[cfg(feature = "KERNEL_MCS")]
    {
        if target.tcbSchedContext != 0 && target.tcbState.get_tcbInReleaseQueue() == 0 {
            if unsafe { ksCurDomain != target.domain } {
                target.sched_enqueue();
            } else if get_ks_scheduler_action() != SchedulerAction_ResumeCurrentThread {
                rescheduleRequired();
                target.sched_enqueue();
            } else {
                set_ks_scheduler_action(target.get_ptr());
            }
        }
    }
}

#[no_mangle]
/// Schedule current thread if time slice is expired.
pub fn timerTick() {
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
            rescheduleRequired();
        }
    }
}

#[no_mangle]
/// Activate the current thread.
pub fn activateThread() {
    let thread = get_currenct_thread();
    // debug!("current: {:#x}", thread.get_ptr());
    #[cfg(feature = "KERNEL_MCS")]
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
            let pc = thread.tcbArch.get_register(ArchReg::FaultIP);
            // setNextPC(thread, pc);
            // sel4_common::println!("restart pc is {:x}",pc);
            thread.tcbArch.set_register(ArchReg::NextIP, pc);
            // set_thread_state(thread, ThreadStateRunning);
            set_thread_state(thread, ThreadState::ThreadStateRunning);
        }
        // 诡异的语法...
        ThreadState::ThreadStateIdleThreadState => return {},
        _ => panic!(
            "current thread is blocked , state id :{}",
            thread.get_state() as usize
        ),
    }
}
#[cfg(feature = "KERNEL_MCS")]
pub fn configure_sched_context(tcb: &mut tcb_t, sc_pptr: &mut sched_context_t, timeslice: ticks_t) {
    tcb.tcbSchedContext = sc_pptr.get_ptr();
    sc_pptr.refill_new(MIN_REFILLS, timeslice, 0);
    sc_pptr.scTcb = tcb.get_ptr();
}

#[cfg(not(feature = "ENABLE_SMP"))]
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
        #[cfg(feature = "KERNEL_MCS")]
        {
            tcb.tcbYieldTo = 0;
            configure_sched_context(
                convert_to_mut_type_ref::<tcb_t>(ksIdleThread),
                convert_to_mut_type_ref::<sched_context_t>(
                    &mut ksIdleThreadSC.data[0][0] as *mut u8 as usize,
                ),
                usToTicks(CONFIG_BOOT_THREAD_TIME_SLICE * US_IN_MS),
            );
            ksIdleSC = &mut ksIdleThreadSC.data[0][0] as *mut u8 as usize;
        }
    }
}

#[cfg(feature = "ENABLE_SMP")]
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
            // TODO mcs support
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
