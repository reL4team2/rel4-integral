use crate::structures::FinaliseCapRet;
use sel4_common::{structures::exception_t, structures_gen::cap};

extern "C" {
    pub fn finalise_cap(capability: &cap, _final: bool, _exposed: bool) -> FinaliseCapRet;

    pub fn post_cap_deletion(capability: &cap);

    pub fn preemption_point() -> exception_t;
}
