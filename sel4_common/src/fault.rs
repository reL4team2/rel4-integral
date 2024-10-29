//! This module defines fault types and related constants for the seL4 microkernel.
//! It provides bitfield definitions for different fault types, such as NullFault, CapFault,
//! UnknownSyscall, UserException, and VMFault.
//!
//! The `FaultType` enum represents the different fault types, and the `seL4_Fault_t` struct
//! provides methods to get the fault type.
//!
//! The module also defines constants for specific fault types, such as `seL4_Fault_NullFault`,
//! `seL4_Fault_CapFault`, `seL4_Fault_UnknownSyscall`, `seL4_Fault_UserException`, and `seL4_Fault_VMFault`.
//!
//! Additionally, it defines constants for specific fields in the `seL4_VMFault_Msg` and `seL4_CapFault_Msg` structs.
//!
//! The `LookupFaultType` enum represents different types of lookup faults, such as InvalidRoot,
//! MissingCap, DepthMismatch, and GuardMismatch. The `lookup_fault_t` struct provides methods
//! to get the lookup fault type.
//!
//! The module also defines constants for specific lookup fault types, such as `lookup_fault_invalid_root`,
//! `lookup_fault_missing_capability`, `lookup_fault_depth_mismatch`, and `lookup_fault_guard_mismatch`.
//!
use crate::plus_define_bitfield;

#[cfg(target_arch = "riscv64")]
plus_define_bitfield! {
    seL4_Fault_t, 2, 0, 0, 4 => {
        new_null_fault, seL4_Fault_NullFault => {},
        new_cap_fault, seL4_Fault_CapFault => {
            address, cap_fault_get_address, cap_fault_set_address, 1, 0, 64, 0, false,
            in_receive_phase, cap_fault_get_in_receive_phase, cap_fault_set_in_receive_phase, 0, 63, 1, 0, false
        },
        new_unknown_syscall_fault, seL4_Fault_UnknownSyscall => {
            syscall_number, unknown_syscall_get_syscall_number, unknown_syscall_set_syscall_number, 1, 0, 64, 0, false
        },
        new_user_exeception, seL4_Fault_UserException => {
            number, user_exeception_get_number, user_exeception_set_number, 0, 32, 32, 0, false,
            code, user_exeception_get_code, user_exeception_set_code, 0, 4, 28, 0, false
        },
        new_vm_fault, seL4_Fault_VMFault => {
            address, vm_fault_get_address, vm_fault_set_address, 1, 0, 64, 0, false,
            fsr, vm_fault_get_fsr, vm_fault_set_fsr, 0, 27, 5, 0, false,
            instruction_fault, vm_fault_get_instruction_fault, vm_fault_set_instruction_fault, 0, 19, 1, 0, false
        }
    }
}

// TODO: Improve seL4_fault_T type
// TIPS: This sel4_fault was defined in bitfield file(2words).
//       sel4_c_impl/include/arch/arm/arch/64/mode/object/structures.bf: VMFault
#[cfg(target_arch = "aarch64")]
plus_define_bitfield! {
    seL4_Fault_t, 2, 0, 0, 4 => {
        new_null_fault, seL4_Fault_NullFault => {},
        new_cap_fault, seL4_Fault_CapFault => {
            address, cap_fault_get_address, cap_fault_set_address, 1, 0, 64, 0, false,
            in_receive_phase, cap_fault_get_in_receive_phase, cap_fault_set_in_receive_phase, 0, 63, 1, 0, false
        },
        new_unknown_syscall_fault, seL4_Fault_UnknownSyscall => {
            syscall_number, unknown_syscall_get_syscall_number, unknown_syscall_set_syscall_number, 1, 0, 64, 0, false
        },
        new_user_exeception, seL4_Fault_UserException => {
            number, user_exeception_get_number, user_exeception_set_number, 0, 32, 32, 0, false,
            code, user_exeception_get_code, user_exeception_set_code, 0, 4, 28, 0, false
        },
        new_vm_fault, seL4_Fault_VMFault => {
            address, vm_fault_get_address, vm_fault_set_address, 1, 0, 64, 0, false,
            fsr, vm_fault_get_fsr, vm_fault_set_fsr, 0, 32, 32, 0, false,
            instruction_fault, vm_fault_get_instruction_fault, vm_fault_set_instruction_fault, 0, 31, 1, 0, false
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultType {
    NullFault = 0,
    CapFault = 1,
    UnknownSyscall = 2,
    UserException = 3,
    VMFault = 5,
}

impl seL4_Fault_t {
    pub fn get_fault_type(&self) -> FaultType {
        unsafe { core::mem::transmute::<u8, FaultType>(self.get_type() as u8) }
    }
}

pub const seL4_Fault_NullFault: usize = FaultType::NullFault as usize;
pub const seL4_Fault_CapFault: usize = FaultType::CapFault as usize;
pub const seL4_Fault_UnknownSyscall: usize = FaultType::UnknownSyscall as usize;
pub const seL4_Fault_UserException: usize = FaultType::UserException as usize;
pub const seL4_Fault_VMFault: usize = FaultType::VMFault as usize;

//seL4_VMFault_Msg
pub const seL4_VMFault_IP: usize = 0;
pub const seL4_VMFault_Addr: usize = 1;
pub const seL4_VMFault_PrefetchFault: usize = 2;
pub const seL4_VMFault_FSR: usize = 3;
pub const seL4_VMFault_Length: usize = 4;

pub const seL4_CapFault_IP: usize = 0;
pub const seL4_CapFault_Addr: usize = 1;
pub const seL4_CapFault_InRecvPhase: usize = 2;
pub const seL4_CapFault_LookupFailureType: usize = 3;
pub const seL4_CapFault_BitsLeft: usize = 4;
pub const seL4_CapFault_DepthMismatch_BitsFound: usize = 5;
pub const seL4_CapFault_GuardMismatch_GuardFound: usize = seL4_CapFault_DepthMismatch_BitsFound;
pub const seL4_CapFault_GuardMismatch_BitsFound: usize = 6;
