use crate::arch::ArchReg;

//     X0                          = 0,    /* 0x00 */
pub const CAP_REGISTER: usize = 0;
pub(super) const BADGE_REEGISTER: usize = 0;
//     X1                          = 1,    /* 0x08 */
pub(super) const MSG_INFO_REGISTER: usize = 1;
//     X2                          = 2,    /* 0x10 */
//     X3                          = 3,    /* 0x18 */
//     X4                          = 4,    /* 0x20 */
//     X5                          = 5,    /* 0x28 */
//     X6                          = 6,    /* 0x30 */
// #ifdef CONFIG_KERNEL_MCS
//     REPLY_REGISTER               = 6,
// #endif
//     X7                          = 7,    /* 0x38 */
//     X8                          = 8,    /* 0x40 */
// #ifdef CONFIG_KERNEL_MCS
//     NB_SEND_RECV_DEST              = 8,
// #endif
#[cfg(feature = "kernel_mcs")]
pub const REPLY_REGISTER: usize = 6;
#[cfg(feature = "kernel_mcs")]
pub const NB_SEND_RECV_DEST: usize = 8;
//     X9                          = 9,    /* 0x48 */
//     X10                         = 10,   /* 0x50 */
//     X11                         = 11,   /* 0x58 */
//     X12                         = 12,   /* 0x60 */
//     X13                         = 13,   /* 0x68 */
//     X14                         = 14,   /* 0x70 */
//     X15                         = 15,   /* 0x78 */
//     X16                         = 16,   /* 0x80 */
//     X17                         = 17,   /* 0x88 */
//     X18                         = 18,   /* 0x90 */
//     X19                         = 19,   /* 0x98 */
//     X20                         = 20,   /* 0xa0 */
//     X21                         = 21,   /* 0xa8 */
//     X22                         = 22,   /* 0xb0 */
//     X23                         = 23,   /* 0xb8 */
//     X24                         = 24,   /* 0xc0 */
//     X25                         = 25,   /* 0xc8 */
//     X26                         = 26,   /* 0xd0 */
//     X27                         = 27,   /* 0xd8 */
//     X28                         = 28,   /* 0xe0 */
//     X29                         = 29,   /* 0xe8 */
//     X30                         = 30,   /* 0xf0 */
//     LR                          = 30,

//     /* End of GP registers, the following are additional kernel-saved state. */
pub(super) const SP_EL0: usize = 31;
pub(super) const ELR_EL1: usize = 32;
pub(super) const NEXT_IP: usize = 32;
pub(super) const SPSR_EL1: usize = 33;
pub(super) const FAULT_IP: usize = 34;
//     /* user readable/writable thread ID register.
//      * name comes from the ARM manual */
pub(super) const TPIDR_EL0: usize = 35;
//     TLS_BASE                    = TPIDR_EL0,
pub(super) const TLS_BASE: usize = TPIDR_EL0;
/// user readonly thread ID register.
pub(super) const TPIDRRO_EL0: usize = 36;
// pub const n_contextRegisters: usize = 37;
// This is n_context registers
pub const CONTEXT_REG_NUM: usize = 37;
pub const N_EXCEPTON_MESSAGE: usize = 3;
pub const N_SYSCALL_MESSAGE: usize = 12;
#[cfg(feature = "kernel_mcs")]
pub const N_TIMEOUT_MESSAGE: usize = 34;
pub const MSG_REGISTER_NUM: usize = 4;
pub const MSG_REGISTER: [usize; MSG_REGISTER_NUM] = [2, 3, 4, 5];
#[cfg(not(feature = "kernel_mcs"))]
pub const MAX_MSG_SIZE: usize = N_SYSCALL_MESSAGE;
#[cfg(feature = "kernel_mcs")]
pub const MAX_MSG_SIZE: usize = N_TIMEOUT_MESSAGE;
#[cfg(not(feature = "kernel_mcs"))]
pub const FAULT_MESSAGES: [[usize; MAX_MSG_SIZE]; 2] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 34, 31, 32, 33],
    [34, 31, 33, 0, 0, 0, 0, 0, 0, 0, 0, 0],
];
#[cfg(feature = "kernel_mcs")]
pub const FAULT_MESSAGES: [[usize; MAX_MSG_SIZE]; 3] = [
    [
        0, 1, 2, 3, 4, 5, 6, 7, 34, 31, 32, 33, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0,
    ],
    [
        34, 31, 33, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0,
    ],
    [
        34, 31, 33, 0, 1, 2, 3, 4, 5, 6, 7, 8, 16, 17, 18, 29, 30, 9, 10, 11, 12, 13, 14, 15, 19,
        20, 21, 22, 23, 24, 25, 26, 27, 28,
    ],
];
pub const FRAME_REG_NUM: usize = 17;
pub const GP_REG_NUM: usize = 19;
pub const FRAME_REGISTERS: [usize; FRAME_REG_NUM] =
    [34, 31, 33, 0, 1, 2, 3, 4, 5, 6, 7, 8, 16, 17, 18, 29, 30];
pub const GP_REGISTERS: [usize; GP_REG_NUM] = [
    9, 10, 11, 12, 13, 14, 15, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 35, 36,
];

impl ArchReg {
    /// Convert Enum to register index.
    pub const fn to_index(&self) -> usize {
        match self {
            ArchReg::TlsBase => 35,
            ArchReg::Cap => 0,
            ArchReg::Badge => 0,
            ArchReg::MsgInfo => 1,
            ArchReg::FAULT_IP => 34,
            ArchReg::NEXT_IP => 32,
            ArchReg::Msg(i) => MSG_REGISTER[*i],
            ArchReg::Frame(i) => FRAME_REGISTERS[*i],
            ArchReg::GP(i) => GP_REGISTERS[*i],
            ArchReg::FaultMessage(id, index) => FAULT_MESSAGES[*id][*index],
            #[cfg(feature = "kernel_mcs")]
            ArchReg::Reply => REPLY_REGISTER,
            #[cfg(feature = "kernel_mcs")]
            ArchReg::nbsRecvDest => NB_SEND_RECV_DEST,
        }
    }
}
