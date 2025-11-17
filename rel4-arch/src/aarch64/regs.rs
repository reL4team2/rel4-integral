use crate::regs::ArchReg;

pub const N_SYSCALL_MESSAGE: usize = 12;

pub const FRAME_REG_NUM: usize = 17;
pub const GP_REG_NUM: usize = 19;

pub const MSG_REGISTER_NUM: usize = 4;
pub const MSG_REGISTER: [usize; MSG_REGISTER_NUM] = [2, 3, 4, 5];

pub const FRAME_REGISTERS: [usize; FRAME_REG_NUM] =
    [34, 31, 33, 0, 1, 2, 3, 4, 5, 6, 7, 8, 16, 17, 18, 29, 30];
pub const GP_REGISTERS: [usize; GP_REG_NUM] = [
    9, 10, 11, 12, 13, 14, 15, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 35, 36,
];

cfg_if::cfg_if! {
    if #[cfg(feature = "mcs")] {
        pub const REPLY_REGISTER: usize = 6;
        pub const NB_SEND_RECV_DEST: usize = 8;
        pub const N_TIMEOUT_MESSAGE: usize = 34;
        pub const MAX_MSG_SIZE: usize = N_TIMEOUT_MESSAGE;

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
    } else {
        pub const MAX_MSG_SIZE: usize = N_SYSCALL_MESSAGE;
        pub const FAULT_MESSAGES: [[usize; MAX_MSG_SIZE]; 2] = [
            [0, 1, 2, 3, 4, 5, 6, 7, 34, 31, 32, 33],
            [34, 31, 33, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ];
    }
}

impl ArchReg {
    /// Convert Enum to register index.
    pub const fn to_index(&self) -> usize {
        match self {
            ArchReg::TlsBase => 35,
            ArchReg::Cap => 0,
            ArchReg::Badge => 0,
            ArchReg::MsgInfo => 1,
            ArchReg::FaultIP => 34,
            ArchReg::NextIP => 32,
            ArchReg::Msg(i) => MSG_REGISTER[*i],
            ArchReg::Frame(i) => FRAME_REGISTERS[*i],
            ArchReg::GP(i) => GP_REGISTERS[*i],
            ArchReg::FaultMessage(id, index) => FAULT_MESSAGES[*id][*index],
            #[cfg(feature = "mcs")]
            ArchReg::Reply => REPLY_REGISTER,
            #[cfg(feature = "mcs")]
            ArchReg::NBSRecvDest => NB_SEND_RECV_DEST,
        }
    }
}
