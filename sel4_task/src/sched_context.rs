use core::{
    intrinsics::{likely, unlikely},
    mem::size_of,
};

use sel4_common::{
    arch::{
        getKernelWcetTicks, getKernelWcetUs, getMaxTicksToUs, getMaxUsToTicks, ticksToUs,
        usToTicks, ArchReg::MsgInfo,
    },
    message_info::seL4_MessageInfo_func,
    platform::time_def::{ticks_t, time_t},
    sel4_config::{CONFIG_KERNEL_WCET_SCALE, UINT64_MAX},
    shared_types_bf_gen::seL4_MessageInfo,
    structures_gen::{cap_sched_context_cap, notification, notification_t},
    utils::{convert_to_mut_type_ref, convert_to_option_mut_type_ref},
    BIT,
};

use crate::{
    get_currenct_thread, get_current_sc, ksCurSC, ksCurTime, ksReprogram, ksSchedulerAction,
    rescheduleRequired, tcb_t,
};

pub type sched_context_t = sched_context;
#[repr(C)]
#[derive(Debug, Clone)]
pub struct sched_context {
    // TODO: MCS
    pub scPeriod: ticks_t,
    pub scConsumed: ticks_t,
    pub scCore: usize,
    pub scTcb: usize,
    pub scReply: usize,
    pub scNotification: usize,
    pub scBadge: usize,
    pub scYieldFrom: usize,
    pub scRefillMax: usize,
    pub scRefillHead: usize,
    pub scRefillTail: usize,
    pub scSporadic: bool,
}
pub const MIN_REFILLS: usize = 2;
pub(crate) type refill_t = refill;
#[repr(C)]
#[derive(Debug, Clone)]
pub struct refill {
    pub rTime: ticks_t,
    pub rAmount: ticks_t,
}
pub fn MIN_BUDGET_US() -> time_t {
    2 * getKernelWcetUs() * CONFIG_KERNEL_WCET_SCALE
}
pub fn MIN_BUDGET() -> time_t {
    2 * getKernelWcetTicks() * CONFIG_KERNEL_WCET_SCALE
}
pub fn MAX_PERIOD_US() -> time_t {
    getMaxUsToTicks() / 8
}
pub fn MAX_RELEASE_TIME() -> time_t {
    UINT64_MAX - 5 * usToTicks(MAX_PERIOD_US())
}
pub fn refill_absolute_max(sc_cap: &cap_sched_context_cap) -> usize {
    return (BIT!(sc_cap.get_capSCSizeBits() as usize) - size_of::<sched_context_t>())
        / size_of::<refill_t>();
}

impl sched_context {
    #[inline]
    pub fn get_ptr(&self) -> usize {
        self as *const sched_context_t as usize
    }
    #[inline]
    pub fn is_round_robin(&self) -> bool {
        self.scPeriod == 0
    }
    #[inline]
    pub fn is_current(&self) -> bool {
        self.get_ptr() == unsafe { ksCurSC }
    }
    #[inline]
    pub fn sc_released(&mut self) -> bool {
        if self.sc_active() {
            assert!(self.refill_sufficient(0));
            return self.refill_ready();
        } else {
            return false;
        }
    }
    #[inline]
    pub fn sc_active(&self) -> bool {
        self.scRefillMax > 0
    }
    #[inline]
    pub fn sc_sporadic(&self) -> bool {
        self.sc_active() && self.scSporadic
    }
    #[inline]
    pub fn postpone(&self) {
        convert_to_mut_type_ref::<tcb_t>(self.scTcb).sched_dequeue();
        convert_to_mut_type_ref::<tcb_t>(self.scTcb).Release_Enqueue();
        unsafe { ksReprogram = true };
    }
    #[inline]
    fn refill_pop_head(&mut self) -> *mut refill_t {
        assert!(!self.refill_single());
        let prev_size = self.refill_size();
        let refill_res = self.refill_head();
        let refill_head = self.scRefillHead;
        self.scRefillHead = self.refill_next(refill_head);
        assert!(prev_size == self.refill_size() + 1);
        assert!(self.scRefillHead < self.scRefillMax);
        refill_res
    }
    #[inline]
    pub fn refill_next(&self, index: usize) -> usize {
        if index == self.scRefillMax - 1 {
            0
        } else {
            index + 1
        }
    }
    #[inline]
    pub fn sc_constant_bandwidth(&mut self) -> bool {
        !self.scSporadic
    }
    #[inline]
    pub fn refill_add_tail(&mut self, rTime: ticks_t, rAmount: ticks_t) {
        assert!(self.refill_size() < self.scRefillMax);
        let new_tail = self.refill_next(self.scRefillTail);
        self.scRefillTail = new_tail;
        unsafe {
            (*self.refill_tail()).rAmount = rAmount;
            (*self.refill_tail()).rTime = rTime;
        }
        assert!(new_tail < self.scRefillMax);
    }
    #[inline]
    pub fn maybe_add_empty_tail(&mut self) {
        if self.is_round_robin() {
            self.refill_add_tail(unsafe { (*self.refill_head()).rTime }, 0);
        }
    }
    #[inline]
    pub fn refill_new(&mut self, max_refills: usize, budget: usize, period: ticks_t) {
        self.scPeriod = period;
        self.scRefillHead = 0;
        self.scRefillTail = 0;
        self.scRefillMax = max_refills;
        assert!(budget >= MIN_BUDGET());
        unsafe {
            (*self.refill_head()).rAmount = budget;
            (*self.refill_head()).rTime = ksCurTime;
        }
        self.maybe_add_empty_tail();
    }
    #[inline]
    pub fn refill_head_overlapping(&mut self) -> bool {
        if !self.refill_single() {
            let amount = unsafe { (*self.refill_head()).rAmount };
            let tail = unsafe { (*self.refill_head()).rTime } + amount;
            return unsafe { (*self.refill_index(self.refill_next(self.scRefillHead))).rTime }
                <= tail;
        } else {
            return false;
        }
    }
    #[inline]
    pub fn refill_unblock_check(&mut self) {
        if self.is_round_robin() {
            return;
        }
        if self.refill_ready() {
            unsafe { (*self.refill_head()).rTime = ksCurTime };
            unsafe { ksReprogram = true };
            while self.refill_head_overlapping() {
                let old_head = self.refill_pop_head();
                unsafe {
                    (*self.refill_head()).rTime = (*old_head).rTime;
                    (*self.refill_head()).rAmount += (*old_head).rAmount;
                }
            }
        }
    }
    #[inline]
    pub fn refill_ready(&mut self) -> bool {
        unsafe { (*self.refill_head()).rTime <= ksCurTime + getKernelWcetTicks() }
    }
    #[inline]
    pub fn refill_index(&self, index: usize) -> *mut refill_t {
        //&mut refill_t {
        convert_to_mut_type_ref::<refill_t>(
            (self.get_ptr() + size_of::<sched_context_t>()) + index * size_of::<refill_t>(),
        ) as *mut refill_t
    }
    #[inline]
    pub fn refill_head(&self) -> *mut refill_t {
        self.refill_index(self.scRefillHead)
    }
    #[inline]
    pub fn refill_tail(&self) -> *mut refill_t {
        self.refill_index(self.scRefillTail)
    }
    pub fn refill_size(&mut self) -> usize {
        if self.scRefillHead <= self.scRefillTail {
            return self.scRefillTail - self.scRefillHead + 1;
        }
        return self.scRefillTail + 1 + (self.scRefillMax - self.scRefillHead);
    }
    pub fn refill_full(&mut self) -> bool {
        self.refill_size() == self.scRefillMax
    }
    pub fn refill_single(&mut self) -> bool {
        self.scRefillHead == self.scRefillTail
    }
    #[inline]
    pub fn refill_capacity(&mut self, usage: ticks_t) -> ticks_t {
        if unlikely(usage > unsafe { (*self.refill_head()).rAmount }) {
            return 0;
        }
        return unsafe { (*self.refill_head()).rAmount } - usage;
    }
    #[inline]
    pub fn refill_sufficient(&mut self, usage: ticks_t) -> bool {
        self.refill_capacity(usage) >= MIN_BUDGET()
    }
    #[inline]
    pub fn refill_update(
        &mut self,
        new_period: ticks_t,
        new_budget: ticks_t,
        new_max_refills: usize,
    ) {
        /* refill must be initialised in order to be updated - otherwise refill_new should be used */
        assert!(self.scRefillMax > 0);

        unsafe {
            (*self.refill_index(0)).rAmount = (*self.refill_head()).rAmount;
            (*self.refill_index(0)).rTime = (*self.refill_head()).rTime;
            self.scRefillHead = 0;
            /* truncate refill list to size 1 */
            self.scRefillTail = self.scRefillHead;
            /* update max refills */
            self.scRefillMax = new_max_refills;
            /* update period */
            self.scPeriod = new_period;

            if self.refill_ready() {
                (*self.refill_head()).rTime = ksCurTime;
            }

            if (*self.refill_head()).rAmount >= new_budget {
                /* if the heads budget exceeds the new budget just trim it */
                (*self.refill_head()).rAmount = new_budget;
                self.maybe_add_empty_tail();
            } else {
                /* otherwise schedule the rest for the next period */
                self.refill_add_tail(
                    (*self.refill_head()).rTime + new_period,
                    new_budget - (*self.refill_head()).rAmount,
                );
            }
        }
    }
    #[inline]
    pub fn schedule_used(&mut self, new_rTime: ticks_t, new_rAmount: ticks_t) {
        // TODO: MCS
        unsafe {
            if unlikely((*self.refill_tail()).rTime + (*self.refill_tail()).rAmount >= new_rTime) {
                (*self.refill_tail()).rAmount += new_rAmount;
            } else if likely(!self.refill_full()) {
                self.refill_add_tail(new_rTime, new_rAmount);
            } else {
                (*self.refill_tail()).rTime = new_rTime - (*self.refill_tail()).rAmount;
                (*self.refill_tail()).rAmount += new_rAmount;
            }
        }
    }

    pub fn schedContext_resume(&mut self) {
        assert!(self.get_ptr() != 0 || self.scTcb != 0);
        if likely(self.get_ptr() != 0)
            && convert_to_mut_type_ref::<tcb_t>(self.scTcb).is_schedulable()
        {
            if !(self.refill_ready() && self.refill_sufficient(0)) {
                assert!(
                    convert_to_mut_type_ref::<tcb_t>(self.scTcb)
                        .tcbState
                        .get_tcbQueued()
                        == 0
                );
                self.postpone();
            }
        }
    }
    pub fn schedContext_bindTCB(&mut self, tcb: &mut tcb_t) {
        assert!(self.scTcb == 0);
        assert!(tcb.tcbSchedContext == 0);
        tcb.tcbSchedContext = self.get_ptr();
        self.scTcb = tcb.get_ptr();
        if self.sc_sporadic() && self.sc_active() && !self.is_current() {
            self.refill_unblock_check()
        }
        self.schedContext_resume();
        if tcb.is_schedulable() {
            tcb.sched_enqueue();
            rescheduleRequired();
        }
    }
    pub fn schedContext_unbindTCB(&mut self, tcb: &mut tcb_t) {
        assert!(self.scTcb == tcb.get_ptr());
        if tcb.is_current() {
            rescheduleRequired();
        }
        convert_to_mut_type_ref::<tcb_t>(self.scTcb).sched_dequeue();
        convert_to_mut_type_ref::<tcb_t>(self.scTcb).Release_Remove();
        convert_to_mut_type_ref::<tcb_t>(self.scTcb).tcbSchedContext = 0;
        self.scTcb = 0;
    }
    pub fn schedContext_unbindAllTCBs(&mut self) {
        if self.scTcb != 0 {
            self.schedContext_unbindTCB(convert_to_mut_type_ref::<tcb_t>(self.scTcb));
        }
    }
    pub fn schedContext_donate(&mut self, to: &mut tcb_t) {
        assert!(self.get_ptr() != 0);
        assert!(to.get_ptr() != 0);
        assert!(to.tcbSchedContext == 0);
        if let Some(from) = convert_to_option_mut_type_ref::<tcb_t>(self.scTcb) {
            from.sched_dequeue();
            from.Release_Remove();
            from.tcbSchedContext = 0;
            if from.is_current() || from.get_ptr() == unsafe { ksSchedulerAction } {
                rescheduleRequired();
            }
        }
        self.scTcb = to.get_ptr();
        to.tcbSchedContext = self.get_ptr()
    }
    pub fn schedContext_bindNtfn(&mut self, ntfn: &mut notification_t) {
        ntfn.set_ntfnSchedContext(self.get_ptr() as u64);
        self.scNotification = ntfn as *mut _ as usize;
    }
    pub fn schedContext_unbindNtfn(&mut self) {
        if self.scNotification != 0 {
            convert_to_mut_type_ref::<notification>(self.scNotification).set_ntfnSchedContext(0);
            self.scNotification = 0;
        }
    }
    pub fn setConsumed(&mut self) {
        let consumed = self.schedContext_updateConsumed();
        let length = get_currenct_thread().set_mr(0, consumed);
        get_currenct_thread().tcbArch.set_register(
            MsgInfo,
            seL4_MessageInfo::new(0, 0, 0, length as u64).to_word(),
        );
    }
    pub fn schedContext_updateConsumed(&mut self) -> time_t {
        let consumed: ticks_t = self.scConsumed;
        if consumed >= getMaxTicksToUs() {
            self.scConsumed -= getMaxTicksToUs();
            return ticksToUs(getMaxTicksToUs());
        } else {
            self.scConsumed = 0;
            return ticksToUs(consumed);
        }
    }
}
pub fn refill_budget_check(_usage: ticks_t) {
    unsafe {
        let mut usage = _usage;
        let sc = get_current_sc();
        assert!(!sc.is_round_robin());

        while (*sc.refill_head()).rAmount <= usage && (*sc.refill_head()).rTime < MAX_RELEASE_TIME()
        {
            usage -= (*sc.refill_head()).rAmount;

            if sc.refill_single() {
                (*sc.refill_head()).rTime += sc.scPeriod;
            } else {
                let old_head = sc.refill_pop_head();
                (*old_head).rTime += sc.scPeriod;
                sc.schedule_used((*old_head).rTime, (*old_head).rAmount);
            }
        }
        if usage > 0 && (*sc.refill_head()).rTime < MAX_RELEASE_TIME() {
            assert!((*sc.refill_head()).rAmount > usage);
            let new_rTime = (*sc.refill_head()).rTime + sc.scPeriod;
            let new_rAmount = usage;

            (*sc.refill_head()).rAmount -= usage;
            (*sc.refill_head()).rTime += usage;
            sc.schedule_used(new_rTime, new_rAmount);
        }
        while (*sc.refill_head()).rAmount < MIN_BUDGET() {
            let head = sc.refill_pop_head();
            (*sc.refill_head()).rAmount += (*head).rAmount;
            (*sc.refill_head()).rTime -= (*head).rAmount;
        }
    }
}
