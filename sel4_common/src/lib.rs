//! This crate contains the common code for the seL4 kernel.
//! Such as the seL4 kernel configuration(`Registers`, `Constants`), the seL4 structures(`MessageInfo`, `ObjectType`, `Error`, 'Exception', 'Fault'), and the seL4 utils(`Logging`, `SBI`).
#![no_std]
#![feature(decl_macro)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(clippy::enum_clike_unportable_variant)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
pub mod arch;
pub mod console;
pub mod fault;
pub mod ffi;
pub mod logging;
pub mod message_info;
pub mod object;
pub mod platform;
#[cfg(feature = "KERNEL_MCS")]
pub mod reply;
pub mod sel4_bitfield_types;
pub mod sel4_config;
pub mod shared_types_bf_gen;
pub mod shared_types_gen;
#[cfg(feature = "ENABLE_SMP")]
pub mod smp;
pub mod structures;
pub mod structures_gen;
pub mod types_gen;
pub mod utils;
pub mod vm_rights;

/// The ffi_call macro, It declares the function and call it
///
/// Usage:
///
/// ```rust
/// ffi_call!(map_kernel_devices);
///
/// // call with arguments
/// // Format is fname(arg_name:type => value) -> ret_type
/// ffi_call!(handleUnknownSyscall, a1:usize => 1);
///
/// ffi_call!(some_function);
///
/// ffi_call!(another_function -> i32);
///
/// ffi_call!(multi_arg_function(a: i32 => 1, b: f64 => 3.14));
///
/// ffi_call!(multi_arg_function_with_return(a: i32 => 1, b: f64 => 3.14) -> i64);
/// ```
pub macro ffi_call {
    ($fname:ident) => {
        {
            extern "C" {
                fn $fname();
            }
            unsafe {
                $fname();
            }
        }
    },
    ($fname:ident->$r:ty) => {
        {
            extern "C" {
                fn $fname() -> $r;
            }
            unsafe {
                $fname()
            }
        }
    },
    ($fname:ident($( $aname:ident:$t:ty=>$v:expr ),*)) => {
        {
            extern "C" {
                fn $fname($($aname:$t),*);
            }
            unsafe {
                $fname($($v),*);
            }
        }
    },
    ($fname:ident($( $aname:ident:$t:ty=>$v:expr ),*)->$r:ty) => {
        {
            extern "C" {
                fn $fname($($aname:$t),*) -> $r;
            }
            unsafe {
                $fname($($v),*)
            }
        }
    },
}

/// The ffi_addr get the address of the ffi function.
pub macro ffi_addr($fname:ident) {{
    extern "C" {
        fn $fname();
    }
    $fname as usize
}}
