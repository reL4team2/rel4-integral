use crate::{tcb_queue_t, tcb_t};

//TODO: MCS
#[no_mangle]
pub extern "C" fn sendIPC() {
    unimplemented!("MCS");
}
#[no_mangle]
pub extern "C" fn installTCBCap() {
    unimplemented!("MCS");
}
#[no_mangle]
pub extern "C" fn tcbSchedDequeue() {
    unimplemented!("MCS");
}
#[no_mangle]
pub extern "C" fn possibleSwitchTo() {
    unimplemented!("MCS");
}
#[no_mangle]
pub extern "C" fn tcbSchedAppend() {
    unimplemented!("MCS");
}
#[no_mangle]
pub extern "C" fn invokeTCB_ThreadControlCaps() {
    unimplemented!("MCS");
}
#[no_mangle]
pub extern "C" fn setMCPriority() {
    unimplemented!("MCS");
}
#[no_mangle]
pub extern "C" fn setPriority() {
    unimplemented!("MCS");
}

extern "C" {
    pub fn tcbEPAppend(tcb: &mut tcb_t, queue: tcb_queue_t)->tcb_queue_t;
}