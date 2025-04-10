//! Constants used in ReL4
#![allow(dead_code)]

use sel4_common::sel4_config; 
// include generated config constants
include!(concat!(env!("OUT_DIR"), "/config.rs"));

pub const PAGE_SIZE_BITS: usize = sel4_config::seL4_PageBits;

pub const MAX_NUM_FREEMEM_REG: usize = 16;
pub const NUM_RESERVED_REGIONS: usize = 3;
pub const MAX_NUM_RESV_REG: usize = MAX_NUM_FREEMEM_REG + NUM_RESERVED_REGIONS;

pub const BI_FRAME_SIZE_BITS: usize = 12;
pub const seL4_ASIDPoolBits: usize = 12;

//IRQConstants
#[cfg(feature = "ENABLE_SMP")]
pub const PLIC_IRQ_OFFSET: usize = 0;
pub const PLIC_MAX_IRQ: usize = 0;

#[cfg(feature = "ENABLE_SMP")]
pub const INTERRUPT_IPI_0: usize = 1;
#[cfg(feature = "ENABLE_SMP")]
pub const INTERRUPT_IPI_1: usize = 2;
#[cfg(feature = "ENABLE_SMP")]
pub const KERNEL_TIMER_IRQ: usize = 3;

#[cfg(all(not(feature = "ENABLE_SMP"), target_arch = "riscv64"))]
pub const KERNEL_TIMER_IRQ: usize = 1;

#[cfg(all(not(feature = "ENABLE_SMP"), target_arch = "aarch64"))]
pub const KERNEL_TIMER_IRQ: usize = 27;

#[cfg(target_arch = "riscv64")]
pub const maxIRQ: usize = KERNEL_TIMER_IRQ;

#[cfg(target_arch = "aarch64")]
pub const maxIRQ: usize = 159;

pub const irqInvalid: usize = 0;
