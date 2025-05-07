#[cfg(feature = "kernel_mcs")]
use sel4_common::sel4_config::seL4_MinSchedContextBits;
use sel4_common::{
    sel4_config::{seL4_TCBBits, CONFIG_MAX_NUM_NODES},
    BIT,
};
#[repr(align(2048))]
pub struct ksIdleThreadTCB_data {
    pub data: [[u8; BIT!(seL4_TCBBits)]; CONFIG_MAX_NUM_NODES],
}

// which should align to BIT!(seL4_MinSchedContextBits)
#[repr(align(128))]
#[cfg(feature = "kernel_mcs")]
pub struct ksIdleThreadSC_data {
    pub data: [[u8; CONFIG_MAX_NUM_NODES]; BIT!(seL4_MinSchedContextBits)],
}

#[no_mangle]
#[link_section = "._idle_thread"]
pub static mut ksIdleThreadTCB: ksIdleThreadTCB_data = ksIdleThreadTCB_data {
    data: [[0; BIT!(seL4_TCBBits)]; CONFIG_MAX_NUM_NODES],
};

#[no_mangle]
#[cfg(feature = "kernel_mcs")]
pub static mut ksIdleThreadSC: ksIdleThreadSC_data = ksIdleThreadSC_data {
    data: [[0; CONFIG_MAX_NUM_NODES]; BIT!(seL4_MinSchedContextBits)],
};

extern "C" {
    #[cfg(feature = "enable_smp")]
    pub fn doMaskReschedule(mask: usize);
}
