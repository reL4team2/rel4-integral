#![no_std]
#![crate_type = "staticlib"]
#![feature(core_intrinsics)]
#![no_main]
#![allow(internal_features)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![feature(alloc_error_handler)]
#![feature(const_nonnull_new)]
#![feature(linkage)]
#![feature(stmt_expr_attributes)]

extern crate core;

#[macro_use]
extern crate rel4_utils;

#[macro_use]
extern crate rel4_arch;

use sel4_common::arch::shutdown;
// mod console;
mod arch;
mod boot;
mod interrupt;
mod kernel;
mod lang_items;
mod object;
mod structures;
mod syscall;
mod utils;

mod compatibility;
mod interfaces_impl;

#[cfg(feature = "enable_smp")]
mod smp;

#[no_mangle]
pub extern "C" fn halt() {
    // Capture the link register before any call clobbers it: for a `bl halt`
    // call this is the caller's return address (who called halt); for a `b halt`
    // tail branch it is whatever the trap left in x30. Look the printed address
    // up in the kernel symbol table to identify the call site.
    let caller: usize;
    unsafe {
        core::arch::asm!("mov {caller}, x30", caller = out(reg) caller);
    }
    log::error!("halt() called from {:#018x}", caller);
    crate::arch::dump_fault_registers();
    shutdown()
}

#[no_mangle]
pub extern "C" fn strnlen(str: *const u8, _max_len: usize) -> usize {
    unsafe {
        let mut c = str;
        let mut ans = 0;
        while (*c) != 0 {
            ans += 1;
            c = c.add(1);
        }
        ans
    }
}
