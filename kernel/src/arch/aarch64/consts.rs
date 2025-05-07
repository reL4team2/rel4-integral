#![allow(non_snake_case)]

pub const ARM_DATA_ABORT: usize = 0;
pub const ARM_PREFETCH_ABORT: usize = 1;
#[allow(dead_code)]
pub const DATA_FAULT: usize = 0;
#[allow(dead_code)]
pub const INSTRUCTION_FAULT: usize = 1;
