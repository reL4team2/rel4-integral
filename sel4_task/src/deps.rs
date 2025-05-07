#[cfg(feature = "kernel_mcs")]
use sel4_common::sel4_config::SEL4_MIN_SCHED_CONTEXT_BITS;
use sel4_common::{
    sel4_config::{CONFIG_MAX_NUM_NODES, SEL4_TCB_BITS},
    BIT,
};
#[repr(align(2048))]
pub struct ksIdleThreadTCB_data {
    pub data: [[u8; BIT!(SEL4_TCB_BITS)]; CONFIG_MAX_NUM_NODES],
}

// which should align to BIT!(SEL4_MIN_SCHED_CONTEXT_BITS)
#[repr(align(128))]
#[cfg(feature = "kernel_mcs")]
pub struct ksIdleThreadSC_data {
    pub data: [[u8; CONFIG_MAX_NUM_NODES]; BIT!(SEL4_MIN_SCHED_CONTEXT_BITS)],
}

#[no_mangle]
#[link_section = "._idle_thread"]
pub static mut ksIdleThreadTCB: ksIdleThreadTCB_data = ksIdleThreadTCB_data {
    data: [[0; BIT!(SEL4_TCB_BITS)]; CONFIG_MAX_NUM_NODES],
};

#[no_mangle]
#[cfg(feature = "kernel_mcs")]
pub static mut ksIdleThreadSC: ksIdleThreadSC_data = ksIdleThreadSC_data {
    data: [[0; CONFIG_MAX_NUM_NODES]; BIT!(SEL4_MIN_SCHED_CONTEXT_BITS)],
};

extern "C" {
    #[cfg(feature = "enable_smp")]
    pub fn doMaskReschedule(mask: usize);
}
