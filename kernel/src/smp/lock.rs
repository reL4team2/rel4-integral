use core::default;
use core::sync::atomic::{fence, Ordering, AtomicPtr};
use sel4_common::structures::{irq_t, irq_to_idx, idx_to_irq, to_irqt};
use sel4_common::arch::config::{IRQ_REMOTE_CALL_IPI, IRQ_RESCHEDULE_IPI};

use sel4_common::sel4_config::*;

#[derive(PartialEq, Copy, Clone)]
pub enum clh_qnode_state {
    CLHState_Granted = 0,
    CLHState_Pending
}

// TODO: L1 Cache page size fixed to 64 bytes
#[repr(align(64))]
#[derive(Copy, Clone)]
struct clh_qnode {
    state: clh_qnode_state,
}

impl clh_qnode {
    pub const fn new() -> Self {
        Self { state: clh_qnode_state::CLHState_Granted }
    }

    fn set_state(&mut self, state: clh_qnode_state) {
        self.state = state
    }

    fn raw_ptr(&mut self) -> *mut clh_qnode {
        self as *mut clh_qnode
    }

    fn state(&self) -> clh_qnode_state {
        self.state
    }
}

#[repr(align(64))]
struct clh_qnode_p {
    node: AtomicPtr<clh_qnode>,
    next: AtomicPtr<clh_qnode>,
    ipi: usize,
}

impl clh_qnode_p {
    pub const fn new() -> Self {
        Self {
            node: AtomicPtr::new(core::ptr::null_mut()),
            next: AtomicPtr::new(core::ptr::null_mut()),
            ipi: 0,
        }
    }
}

#[repr(align(64))]
pub struct clh_lock {
    nodes: [clh_qnode; CONFIG_MAX_NUM_NODES + 1],
    node_owners: [clh_qnode_p; CONFIG_MAX_NUM_NODES],
    head: AtomicPtr<clh_qnode>,
}

impl clh_lock {
    pub const fn new() -> Self {
        const OWNER: clh_qnode_p = clh_qnode_p::new();
        Self {
            nodes: [clh_qnode::new(); CONFIG_MAX_NUM_NODES + 1],
            node_owners: [OWNER; CONFIG_MAX_NUM_NODES],
            head: AtomicPtr::new(core::ptr::null_mut()),
        }
    }

    pub fn init(&mut self) {
        for i in 0..CONFIG_MAX_NUM_NODES {
            self.node_owners[i].node.store(self.nodes[i].raw_ptr(), Ordering::Release);
        }
        self.nodes[CONFIG_MAX_NUM_NODES].set_state(clh_qnode_state::CLHState_Granted);
        self.head.store(self.nodes[CONFIG_MAX_NUM_NODES].raw_ptr(), Ordering::Release);
    }

    #[inline]
    pub fn is_ipi_pending(&self, cpu: usize) -> bool {
        self.node_owners[cpu].ipi == 1
    }

    #[inline]
    pub fn is_self_in_queue(&self) -> bool {
        let cpu = sel4_common::utils::cpu_id();
        let value = unsafe {self.node_owners[cpu].node.load(Ordering::Acquire).read().state()};
        value == clh_qnode_state::CLHState_Pending
    }

    #[inline]
    pub fn acquire(&mut self, cpu: usize, irq_path: bool) {
        unsafe {
            self.node_owners[cpu].node.load(Ordering::Acquire).as_mut().unwrap().set_state(clh_qnode_state::CLHState_Pending);
            while true {
                match self.head.compare_exchange(
                    self.head.load(Ordering::Acquire),
                    self.node_owners[cpu].node.load(Ordering::Acquire),
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                ) {
                    Ok(old) => {
                        self.node_owners[cpu].next.store(old, Ordering::Release);
                        while self.node_owners[cpu].next.load(Ordering::Acquire).as_mut().unwrap().state() != clh_qnode_state::CLHState_Granted {
                            if self.is_ipi_pending(cpu) {
                                super::ipi::handle_ipi(IRQ_REMOTE_CALL_IPI, irq_path);
                            }
                            crate::arch::arch_pause();
                        } 
                        break;
                    }
                    Err(_) => {
                        if self.is_ipi_pending(cpu) {
                            super::ipi::handle_ipi(IRQ_REMOTE_CALL_IPI, irq_path);
                        }
                        crate::arch::arch_pause();
                    }
                }
            }
        }
    }

    #[inline]
    pub fn set_ipi(&mut self, cpu: usize, ipi: usize) {
        self.node_owners[cpu].ipi = ipi;
    }

    #[inline]
    pub fn next_node_value(&mut self, cpu: usize) -> clh_qnode_state {
        let next = self.node_owners[cpu].next.load(Ordering::Acquire);
        if next.is_null() {
            return clh_qnode_state::CLHState_Granted;
        }
        unsafe { (*next).state }
    }

    #[inline]
    pub fn release(&mut self, cpu: usize) {
        fence(Ordering::Release);
        unsafe {
            self.node_owners[cpu].node.load(Ordering::Acquire).as_mut().unwrap().set_state(clh_qnode_state::CLHState_Granted);
            let next = self.node_owners[cpu].next.load(Ordering::Acquire);
            self.node_owners[cpu].node.store(next, Ordering::Release);
        }
    }
}
