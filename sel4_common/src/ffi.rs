use crate::sel4_bitfield_types::Bitfield;
use crate::sel4_config::{CONFIG_KERNEL_STACK_BITS, CONFIG_MAX_NUM_NODES};
use crate::structures_gen::seL4_Fault;
use crate::BIT;
#[repr(align(4096))]
pub struct kernel_stack_alloc_data {
    pub data: [[u8; BIT!(CONFIG_KERNEL_STACK_BITS)]; CONFIG_MAX_NUM_NODES],
}
#[no_mangle]
pub static mut kernel_stack_alloc: kernel_stack_alloc_data = kernel_stack_alloc_data {
    data: [[0_u8; BIT!(CONFIG_KERNEL_STACK_BITS)]; CONFIG_MAX_NUM_NODES],
};
#[cfg(feature = "ENABLE_SMP")]
/// This function is used to map the core.
extern "C" {
    pub fn coreMap();
}

#[no_mangle]
// #[link_section = ".boot.bss"]
pub static mut current_fault: seL4_Fault = seL4_Fault {
    0: Bitfield { arr: [0; 2usize] },
};
