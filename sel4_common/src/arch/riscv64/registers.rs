//! This module contains constants representing register indices and values used in the kernel.

use crate::arch::ArchReg;
pub(super) const ra: usize = 0;
pub(super) const sp: usize = 1;
// const gp: usize = 2;
// const tp: usize = 3;
pub(super) const TLS_BASE: usize = 3;
// const t0: usize = 4;
// #ifdef CONFIG_KERNEL_MCS
//     NB_SEND_RECV_DEST = 4,
// #endif
#[cfg(feature = "kernel_mcs")]
pub const NB_SEND_RECV_DEST: usize = 4;
// const t1: usize = 5;
// const t2: usize = 6;
// const s0: usize = 7;
// const s1: usize = 8;
// const a0: usize = 9;
pub(super) const CAP_REGISTER: usize = 9;
pub(super) const BADGE_REEGISTER: usize = 9;
pub(super) const MSG_INFO_REGISTER: usize = 10;
// const a1: usize = 10;
// const a2: usize = 11;
// const a3: usize = 12;
// const a4: usize = 13;
// const a5: usize = 14;
// const a6: usize = 15;
// #ifdef CONFIG_KERNEL_MCS
//     REPLY_REGISTER = 15,
// #endif
#[cfg(feature = "kernel_mcs")]
pub const REPLY_REGISTER: usize = 15;
// const a7: usize = 16;
// const s2: usize = 17;
// const s3: usize = 18;
// const s4: usize = 19;
// const s5: usize = 20;
// const s6: usize = 21;
// const s7: usize = 22;
// const s8: usize = 23;
// const s9: usize = 24;
// const s10: usize = 25;
// const s11: usize = 26;
// const t3: usize = 27;
// const t4: usize = 28;
// const t5: usize = 29;
// const t6: usize = 30;

// Platform specific Register.
pub(super) const SCAUSE: usize = 31;
pub(super) const SSTATUS: usize = 32;

pub(super) const FAULT_IP: usize = 33;
pub(super) const NEXT_IP: usize = 34;
// pub const n_contextRegisters: usize = 35;
// This is n_context registers
pub(super) const CONTEXT_REG_NUM: usize = 35;
pub const MSG_REGISTER_NUM: usize = 4;
pub const MSG_REGISTER: [usize; MSG_REGISTER_NUM] = [11, 12, 13, 14];

pub const SSTATUS_SPIE: usize = 0x00000020;
pub const SSTATUS_SPP: usize = 0x00000100;

pub const N_SYSCALL_MESSAGE: usize = 10;
pub const N_EXCEPTON_MESSAGE: usize = 2;
#[cfg(feature = "kernel_mcs")]
pub const N_TIMEOUT_MESSAGE: usize = 32;
#[cfg(not(feature = "kernel_mcs"))]
pub const MAX_MSG_SIZE: usize = N_SYSCALL_MESSAGE;
#[cfg(feature = "kernel_mcs")]
pub const MAX_MSG_SIZE: usize = N_TIMEOUT_MESSAGE;
#[cfg(not(feature = "kernel_mcs"))]
pub const FAULT_MESSAGES: [[usize; MAX_MSG_SIZE]; 2] = [
    [33, 1, 0, 9, 10, 11, 12, 13, 14, 15],
    [33, 1, 0, 0, 0, 0, 0, 0, 0, 0],
];
#[cfg(feature = "kernel_mcs")]
pub const FAULT_MESSAGES: [[usize; MAX_MSG_SIZE]; 3] = [
    [
        33, 1, 0, 9, 10, 11, 12, 13, 14, 15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    [
        33, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ],
    [
        33, 0, 1, 2, 7, 8, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 9, 10, 11, 12, 13, 14, 15, 16,
        4, 5, 6, 27, 28, 29, 30, 3,
    ],
];

pub const FRAME_REG_NUM: usize = 16;
pub const GP_REG_NUM: usize = 16;

pub const FRAME_REGISTERS: [usize; FRAME_REG_NUM] =
    [33, 0, 1, 2, 7, 8, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26];
pub const GP_REGISTERS: [usize; GP_REG_NUM] =
    [9, 10, 11, 12, 13, 14, 15, 16, 4, 5, 6, 27, 28, 29, 30, 3];

impl ArchReg {
    /// Convert Enum to register index.
    pub const fn to_index(&self) -> usize {
        match self {
            ArchReg::TlsBase => 3,
            ArchReg::Cap => 9,
            ArchReg::Badge => 9,
            ArchReg::MsgInfo => 10,
            ArchReg::FAULT_IP => 33,
            ArchReg::NEXT_IP => 34,
            ArchReg::Msg(i) => MSG_REGISTER[*i],
            ArchReg::Frame(i) => FRAME_REGISTERS[*i],
            ArchReg::GP(i) => GP_REGISTERS[*i],
            ArchReg::FaultMessage(id, index) => FAULT_MESSAGES[*id][*index],
            #[cfg(feature = "kernel_mcs")]
            ArchReg::Reply => REPLY_REGISTER,
            #[cfg(feature = "kernel_mcs")]
            ArchReg::nbsRecvDest => NB_SEND_RECV_DEST,
            ArchReg::SSTATUS => SSTATUS,
        }
    }
}
