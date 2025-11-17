use crate::sel4_bitfield_types::Bitfield;
use crate::sel4_config::{CONFIG_KERNEL_STACK_BITS, CONFIG_MAX_NUM_NODES};
use crate::structures_gen::seL4_Fault;
use rel4_utils::no_lock::NoLock;

#[no_mangle]
pub static kernel_stack_alloc: NoLock<KernelStack> = NoLock::new(KernelStack::new());

#[no_mangle]
// #[link_section = ".boot.bss"]
pub static mut current_fault: seL4_Fault = seL4_Fault {
    0: Bitfield { arr: [0; 2usize] },
};

#[repr(align(4096))]
pub struct KernelStack([[u8; bit!(CONFIG_KERNEL_STACK_BITS)]; CONFIG_MAX_NUM_NODES]);

impl KernelStack {
    pub const fn new() -> Self {
        Self([[0; bit!(CONFIG_KERNEL_STACK_BITS)]; CONFIG_MAX_NUM_NODES])
    }

    /// Get stack top address by node id number
    pub fn get_stack_top(&self, node_id: usize) -> usize {
        self.0[node_id].as_ptr() as usize + bit!(CONFIG_KERNEL_STACK_BITS)
    }

    pub const fn base_ptr(&self) -> *const u8 {
        self.0.as_ptr() as _
    }
}
