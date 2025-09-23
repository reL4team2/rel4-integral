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

use rel4_arch::basic::{PAddr, PRegion};
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

use boot::interface::rust_try_init_kernel;
use sel4_task::{activateThread, schedule};

#[no_mangle]
pub extern "C" fn halt() {
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

#[no_mangle]
#[link_section = ".boot.text"]
#[cfg(all(feature = "build_binary", not(feature = "enable_smp")))]
pub fn init_kernel(
    ui_p_reg_start: PAddr,
    ui_p_reg_end: PAddr,
    pv_offset: isize,
    v_entry: usize,
    dtb_addr_p: PAddr,
    dtb_size: usize,
) {
    use sel4_common::platform::avail_p_regs;
    boot::interface::pRegsToR(
        &avail_p_regs as *const PRegion as *const usize,
        core::mem::size_of_val(&avail_p_regs) / core::mem::size_of::<PRegion>(),
    );

    let result = rust_try_init_kernel(
        ui_p_reg_start,
        ui_p_reg_end,
        pv_offset,
        v_entry,
        dtb_addr_p,
        dtb_size,
    );
    if !result {
        log::error!("ERROR: kernel init failed");
        panic!()
    }

    schedule();
    activateThread();
}

#[no_mangle]
#[link_section = ".boot.text"]
#[cfg(all(
    feature = "build_binary",
    feature = "enable_smp",
    target_arch = "aarch64"
))]
pub fn init_kernel(
    ui_p_reg_start: PAddr,
    ui_p_reg_end: PAddr,
    pv_offset: isize,
    v_entry: usize,
    dtb_addr_p: PAddr,
    dtb_size: usize,
) {
    use sel4_common::platform::avail_p_regs;
    use sel4_common::utils::cpu_id;
    boot::interface::pRegsToR(
        &avail_p_regs as *const PRegion as *const usize,
        core::mem::size_of_val(&avail_p_regs) / core::mem::size_of::<PRegion>(),
    );

    if cpu_id() == 0 {
        let result = rust_try_init_kernel(
            ui_p_reg_start,
            ui_p_reg_end,
            pv_offset,
            v_entry,
            dtb_addr_p,
            dtb_size,
        );
        if !result {
            log::error!("ERROR: kernel init failed");
            panic!()
        }
    } else {
        boot::interface::rust_try_init_kernel_secondary_core(cpu_id(), cpu_id());
    }

    schedule();
    activateThread();
}

#[no_mangle]
#[link_section = ".boot.text"]
#[cfg(all(
    feature = "build_binary",
    feature = "enable_smp",
    target_arch = "riscv64"
))]
pub fn init_kernel(
    ui_p_reg_start: PAddr,
    ui_p_reg_end: PAddr,
    pv_offset: isize,
    v_entry: usize,
    dtb_addr_p: PAddr,
    dtb_size: usize,
    hart_id: usize,
    core_id: usize,
) {
    use sel4_common::platform::avail_p_regs;
    boot::interface::pRegsToR(
        &avail_p_regs as *const PRegion as *const usize,
        core::mem::size_of_val(&avail_p_regs) / core::mem::size_of::<PRegion>(),
    );

    sel4_common::arch::add_hart_to_core_map(hart_id, core_id);
    if core_id == 0 {
        let result = rust_try_init_kernel(
            ui_p_reg_start,
            ui_p_reg_end,
            pv_offset,
            v_entry,
            dtb_addr_p,
            dtb_size,
        );
        if !result {
            log::error!("ERROR: kernel init failed");
            panic!()
        }
    } else {
        boot::interface::rust_try_init_kernel_secondary_core(hart_id, core_id);
    }

    schedule();
    activateThread();
}
