pub mod lock;
pub mod ipi;

use sel4_common::sel4_config::CONFIG_MAX_NUM_NODES;

pub static mut big_kernel_lock: lock::clh_lock = lock::clh_lock::new();

pub fn clh_lock_init() {
    unsafe {
        big_kernel_lock.init();
    }
}

pub fn clh_is_ipi_pending(cpu: usize) -> bool {
    assert!(cpu < CONFIG_MAX_NUM_NODES);
    unsafe { big_kernel_lock.is_ipi_pending(cpu) }
}

pub fn clh_is_self_in_queue() -> bool {
    unsafe { big_kernel_lock.is_self_in_queue() }
}

pub fn clh_set_ipi(cpu: usize, ipi: usize) {
    assert!(cpu < CONFIG_MAX_NUM_NODES);
    unsafe { big_kernel_lock.set_ipi(cpu, ipi) }
}

pub fn clh_next_node_state(cpu: usize) -> lock::clh_qnode_state {
    assert!(cpu < CONFIG_MAX_NUM_NODES);
    unsafe { big_kernel_lock.next_node_value(cpu) }
}

pub fn clh_lock_acquire(cpu: usize, irq_path: bool) {
    assert!(cpu < CONFIG_MAX_NUM_NODES);
    unsafe { big_kernel_lock.acquire(cpu, irq_path) }
}

pub fn clh_lock_release(cpu: usize) {
    assert!(cpu < CONFIG_MAX_NUM_NODES);
    unsafe { big_kernel_lock.release(cpu) }
}

pub fn migrate_tcb(tcb: &mut sel4_task::tcb_t, new_core: usize) {
    // TODO: implement Arch_migrateTCB for arm and riscv
    tcb.tcbAffinity = new_core;
}