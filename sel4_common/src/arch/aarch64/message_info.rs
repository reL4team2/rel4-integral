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
    #[cfg(feature = "KERNEL_MCS")]
    TCBSetTimeoutEndpoint,
    TCBSetIPCBuffer,
    TCBSetSpace,
    TCBSuspend,
    TCBResume,
    TCBBindNotification,
    TCBUnbindNotification,
    #[cfg(feature = "ENABLE_SMP")]
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
    #[cfg(not(feature = "KERNEL_MCS"))]
    CNodeSaveCaller,
    IRQIssueIRQHandler,
    IRQAckIRQ,
    IRQSetIRQHandler,
    IRQClearIRQHandler,
    DomainSetSet,
    #[cfg(feature = "KERNEL_MCS")]
    SchedControlConfigureFlags,
    #[cfg(feature = "KERNEL_MCS")]
    SchedContextBind,
    #[cfg(feature = "KERNEL_MCS")]
    SchedContextUnbind,
    #[cfg(feature = "KERNEL_MCS")]
    SchedContextUnbindObject,
    #[cfg(feature = "KERNEL_MCS")]
    SchedContextConsumed,
    #[cfg(feature = "KERNEL_MCS")]
    SchedContextYieldTo,
    ARMVSpaceClean_Data,
    ARMVSpaceInvalidate_Data,
    ARMVSpaceCleanInvalidate_Data,
    ARMVSpaceUnify_Instruction,
    ARMSMCCall,
    ARMPageTableMap,
    ARMPageTableUnmap,
    ARMPageMap,
    ARMPageUnmap,
    ARMPageClean_Data,
    ARMPageInvalidate_Data,
    ARMPageCleanInvalidate_Data,
    ARMPageUnify_Instruction,
    ARMPageGetAddress,
    ARMASIDControlMakePool,
    ARMASIDPoolAssign,
    ARMIRQIssueIRQHandlerTrigger,
    nArchInvocationLabels,
}
#[cfg(not(feature = "KERNEL_MCS"))]
pub const CNODE_LAST_INVOCATION: usize = MessageLabel::CNodeSaveCaller as usize;
#[cfg(feature = "KERNEL_MCS")]
pub const CNODE_LAST_INVOCATION: usize = MessageLabel::CNodeRotate as usize;
