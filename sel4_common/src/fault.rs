//seL4_VMFault_Msg
pub const VM_FAULT_IP: usize = 0;
pub const VM_FAULT_ADDR: usize = 1;
pub const VM_FAULT_PREFETCH_FAULT: usize = 2;
pub const VM_FAULT_FSR: usize = 3;
pub const VM_FAULT_LENGTH: usize = 4;
pub const TIMEOUT_DATA: usize = 0;

pub const CAP_FAULT_IP: usize = 0;
pub const CAP_FAULT_ADDR: usize = 1;
pub const CAP_FAULT_IN_RECV_PHASE: usize = 2;
pub const CAP_FAULT_LOOKUP_FAILURE_TYPE: usize = 3;
pub const CAP_FAULT_BITS_LEFT: usize = 4;
pub const CAP_FAULT_DEPTH_MISMATCH_BITS_FOUND: usize = 5;
pub const CAP_FAULT_GUARD_MISMATCH_GUARD_FOUND: usize = CAP_FAULT_DEPTH_MISMATCH_BITS_FOUND;
pub const CAP_FAULT_GUARD_MISMATCH_BITS_FOUND: usize = 6;
