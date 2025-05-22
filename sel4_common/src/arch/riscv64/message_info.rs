#[derive(Eq, PartialEq, Debug, Clone, Copy, PartialOrd, Ord)]
/// The label of a message.
#[repr(C)]
pub enum MessageLabel {
    InvalidInvocation = 0,
    UntypedRetype,
    TCBReadRegisters,
    TCBWriteRegisters,
    TCBCopyRegisters,
    TCBConfigure,
    TCBSetPriority,
    TCBSetMCPriority,
    TCBSetSchedParams,
    #[cfg(feature = "kernel_mcs")]
    TCBSetTimeoutEndpoint,
    TCBSetIPCBuffer,
    TCBSetSpace,
    TCBSuspend,
    TCBResume,
    TCBBindNotification,
    TCBUnbindNotification,
    #[cfg(all(feature = "enable_smp", not(feature = "kernel_mcs")))]
    TCBSetAffinity,
    TCBSetTLSBase,
    CNodeRevoke,
    CNodeDelete,
    CNodeCancelBadgedSends,
    CNodeCopy,
    CNodeMint,
    CNodeMove,
    CNodeMutate,
    CNodeRotate,
    #[cfg(not(feature = "kernel_mcs"))]
    CNodeSaveCaller,
    IRQIssueIRQHandler,
    IRQAckIRQ,
    IRQSetIRQHandler,
    IRQClearIRQHandler,
    DomainSetSet,
    #[cfg(feature = "kernel_mcs")]
    SchedControlConfigureFlags,
    #[cfg(feature = "kernel_mcs")]
    SchedContextBind,
    #[cfg(feature = "kernel_mcs")]
    SchedContextUnbind,
    #[cfg(feature = "kernel_mcs")]
    SchedContextUnbindObject,
    #[cfg(feature = "kernel_mcs")]
    SchedContextConsumed,
    #[cfg(feature = "kernel_mcs")]
    SchedContextYieldTo,
    RISCVPageTableMap,
    RISCVPageTableUnmap,
    RISCVPageMap,
    RISCVPageUnmap,
    RISCVPageGetAddress,
    RISCVASIDControlMakePool,
    RISCVASIDPoolAssign,
    RISCVIRQIssueIRQHandlerTrigger,
    nArchInvocationLabels,
}
#[cfg(not(feature = "kernel_mcs"))]
pub const CNODE_LAST_INVOCATION: usize = MessageLabel::CNodeSaveCaller as usize;
#[cfg(feature = "kernel_mcs")]
pub const CNODE_LAST_INVOCATION: usize = MessageLabel::CNodeRotate as usize;
