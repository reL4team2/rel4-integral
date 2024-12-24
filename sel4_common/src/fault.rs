//seL4_VMFault_Msg
pub const seL4_VMFault_IP: usize = 0;
pub const seL4_VMFault_Addr: usize = 1;
pub const seL4_VMFault_PrefetchFault: usize = 2;
pub const seL4_VMFault_FSR: usize = 3;
pub const seL4_VMFault_Length: usize = 4;
pub const seL4_Timeout_Data: usize = 0;

pub const seL4_CapFault_IP: usize = 0;
pub const seL4_CapFault_Addr: usize = 1;
pub const seL4_CapFault_InRecvPhase: usize = 2;
pub const seL4_CapFault_LookupFailureType: usize = 3;
pub const seL4_CapFault_BitsLeft: usize = 4;
pub const seL4_CapFault_DepthMismatch_BitsFound: usize = 5;
pub const seL4_CapFault_GuardMismatch_GuardFound: usize = seL4_CapFault_DepthMismatch_BitsFound;
pub const seL4_CapFault_GuardMismatch_BitsFound: usize = 6;
