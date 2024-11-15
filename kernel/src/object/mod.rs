use crate::structures::lookupCapAndSlot_ret_t;
use crate::syscall::handle_fault;
use sel4_common::arch::MessageLabel;
use sel4_common::structures::exception_t;
use sel4_common::structures_gen::{cap, cap_null_cap};
use sel4_cspace::interface::cte_t;
use sel4_task::tcb_t;

#[no_mangle]
pub fn decodeRISCVMMUInvocation(
    _label: MessageLabel,
    _length: usize,
    _cptr: usize,
    _cte: *mut cte_t,
    _cap: &mut cap,
    _call: bool,
    _buffer: *mut usize,
) -> exception_t {
    panic!("should not be invoked!")
}

#[no_mangle]
pub fn configureIdleThread(_tcb: *const tcb_t) {
    panic!("should not be invoked!")
}

#[no_mangle]
pub fn setMR(
    _receiver: *mut tcb_t,
    _receivedBuffer: *mut usize,
    _offset: usize,
    _reg: usize,
) -> usize {
    panic!("should not be invoked!")
}

#[no_mangle]
pub fn handleFault(tptr: *mut tcb_t) {
	//TODO: MCS
    unsafe {
        handle_fault(&mut *tptr);
    }
}

#[no_mangle]
pub extern "C" fn lookupCapAndSlot(thread: *mut tcb_t, cPtr: usize) -> lookupCapAndSlot_ret_t {
    // let lu_ret = lookupSlot(thread, cPtr);
    let lu_ret = unsafe { (*thread).lookup_slot(cPtr) };
    if lu_ret.status != exception_t::EXCEPTION_NONE {
        let ret = lookupCapAndSlot_ret_t {
            status: lu_ret.status,
            slot: 0 as *mut cte_t,
            capability: cap_null_cap::new().unsplay(),
        };
        return ret;
    }
    unsafe {
        let ret = lookupCapAndSlot_ret_t {
            status: exception_t::EXCEPTION_NONE,
            slot: lu_ret.slot,
            capability: (*lu_ret.slot).capability.clone(),
        };
        ret
    }
}
