#![no_std]
#![crate_type = "staticlib"]
#![feature(core_intrinsics)]
#![feature(const_option)]
#![feature(const_nonnull_new)]
#![no_main]
#![allow(internal_features)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![feature(alloc_error_handler)]
#![feature(asm_const)]
#![feature(panic_info_message)]
#![feature(linkage)]
#![feature(stmt_expr_attributes)]

extern crate core;
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
pub use sel4_common::{BIT, IS_ALIGNED, MASK, ROUND_DOWN, ROUND_UP};
use sel4_task::{activateThread, schedule};
use structures::p_region_t;

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
    ui_p_reg_start: usize,
    ui_p_reg_end: usize,
    pv_offset: isize,
    v_entry: usize,
    dtb_addr_p: usize,
    dtb_size: usize,
) {
    use sel4_common::platform::avail_p_regs;
    // sel4_common::println!("Now we use rel4 kernel binary");
    log::set_max_level(log::LevelFilter::Info);
    boot::interface::pRegsToR(
        &avail_p_regs as *const p_region_t as *const usize,
        core::mem::size_of_val(&avail_p_regs) / core::mem::size_of::<p_region_t>(),
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
    ui_p_reg_start: usize,
    ui_p_reg_end: usize,
    pv_offset: isize,
    v_entry: usize,
    dtb_addr_p: usize,
    dtb_size: usize,
) {
    use sel4_common::platform::avail_p_regs;
    use sel4_common::utils::cpu_id;
    // sel4_common::println!("Now we use rel4 kernel binary");
    log::set_max_level(log::LevelFilter::Info);
    boot::interface::pRegsToR(
        &avail_p_regs as *const p_region_t as *const usize,
        core::mem::size_of_val(&avail_p_regs) / core::mem::size_of::<p_region_t>(),
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
    ui_p_reg_start: usize,
    ui_p_reg_end: usize,
    pv_offset: isize,
    v_entry: usize,
    dtb_addr_p: usize,
    dtb_size: usize,
    hart_id: usize,
    core_id: usize,
) {
    use sel4_common::platform::avail_p_regs;
    // sel4_common::println!("Now we use rel4 kernel binary");
    log::set_max_level(log::LevelFilter::Info);
    boot::interface::pRegsToR(
        &avail_p_regs as *const p_region_t as *const usize,
        core::mem::size_of_val(&avail_p_regs) / core::mem::size_of::<p_region_t>(),
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
