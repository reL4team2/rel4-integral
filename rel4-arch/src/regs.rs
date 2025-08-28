/// Arch Register Shared part
/// If not shared. Just Write in the [arch] module.
#[repr(usize)]
#[derive(Debug, Clone)]
pub enum ArchReg {
    /// Generic registers
    TlsBase,
    Cap,
    Badge,
    MsgInfo,
    FaultIP,
    NextIP,
    /// Message Registers Msg(offset)
    Msg(usize),
    /// Frame Registers Frame(Offset)
    Frame(usize),
    /// GPRegisters GP(offset)
    GP(usize),
    /// Fault Message Reg, (id, index)
    FaultMessage(usize, usize),
    #[cfg(feature = "mcs")]
    Reply,
    #[cfg(feature = "mcs")]
    nbsRecvDest,
    #[cfg(target_arch = "riscv64")]
    SSTATUS,
}
