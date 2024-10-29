use crate::structures::finaliseCap_ret;
use sel4_common::{structures::exception_t, structures_gen::cap};

extern "C" {
    pub fn finaliseCap(capability: &cap, _final: bool, _exposed: bool) -> finaliseCap_ret;

    pub fn post_cap_deletion(capability: &cap);

    pub fn preemptionPoint() -> exception_t;
}
