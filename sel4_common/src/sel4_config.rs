//! This module contains the configuration settings for sel4_common.
use crate::BIT;

// include generated config constants
include!(concat!(env!("OUT_DIR"), "/config.rs"));

/// kernel/include/stdint.h
pub const UINT64_MAX: usize = 0xFFFFFFFFFFFFFFFF;

/// kernel/include/arch/{arch}/arch/64/mode/types.h
pub const wordRadix: usize = 6;
pub const wordBits: usize = BIT!(wordRadix);

/// kernel/include/arch/riscv/arch/object/structures.h
pub const PT_SIZE_BITS: usize = 12;
pub const PT_INDEX_BITS: usize = 9;
pub const PD_INDEX_BITS: usize = 9;
pub const UPUD_INDEX_BITS: usize = 9;
pub const PUD_INDEX_BITS: usize = 9;
pub const PGD_INDEX_BITS: usize = 9;

/// kernel/include/arch/riscv/arch/machine/hardware.h
pub const PAGE_BITS: usize = seL4_PageBits;
pub const RISCV_4K_Page: usize = 0;
pub const RISCV_Mega_Page: usize = 1;
pub const RISCV_Giga_Page: usize = 2;
pub const RISCV_Tera_Page: usize = 3;
pub const RISCVPageBits: usize = seL4_PageBits;
pub const RISCVMegaPageBits: usize = seL4_LargePageBits;
pub const RISCVGigaPageBits: usize = seL4_HugePageBits;
pub const RISCVInstructionMisaligned: usize = 0;
pub const RISCVInstructionAccessFault: usize = 1;
pub const RISCVInstructionIllegal: usize = 2;
pub const RISCVBreakPoint: usize = 3;
pub const RISCVLoadAccessFault: usize = 5;
pub const RISCVAddressMisaligned: usize = 6;
pub const RISCVStoreAccessFault: usize = 7;
pub const RISCVEnvCall: usize = 8;
pub const RISCVInstructionPageFault: usize = 12;
pub const RISCVLoadPageFault: usize = 13;
pub const RISCVStorePageFault: usize = 15;

/// kernel/include/arch/arm/arch/64/mode/machine/hardware.h
/// enum vm_page_size {
///     ARMSmallPage,
///     ARMLargePage,
///     ARMHugePage
///};
pub const ARM_Small_Page: usize = 0;
pub const ARM_Large_Page: usize = 1;
pub const ARM_Huge_Page: usize = 2;
pub const ARMSmallPageBits: usize = seL4_PageBits;
pub const ARMLargePageBits: usize = seL4_LargePageBits;
pub const ARMHugePageBits: usize = seL4_HugePageBits;

/// kernel/include/arch/{arch}/mode/object/structures.h
/// kernel/include/arch/arm/arch/64/mode/object/structures.h
pub const PT_INDEX_OFFSET: usize = seL4_PageBits;
pub const PD_INDEX_OFFSET: usize = PT_INDEX_OFFSET + PT_INDEX_BITS;
pub const PUD_INDEX_OFFSET: usize = PD_INDEX_OFFSET + PD_INDEX_BITS;
pub const PGD_INDEX_OFFSET: usize = PUD_INDEX_OFFSET + PUD_INDEX_BITS;
pub const nASIDPools: usize = BIT!(asidHighBits);
pub const ASID_BITS: usize = asidHighBits + asidLowBits;

/// kernel/include/arch/{arch}/arch/api/types.h
pub const asidInvalid: usize = 0;

/// kernel/include/arch/{arch}/arch/kernel/vspace.h
pub const IT_ASID: usize = 1;

/// kernel/include/model
pub const L2_BITMAP_SIZE: usize = (CONFIG_NUM_PRIORITIES + wordBits - 1) / wordBits;
pub const NUM_READY_QUEUES: usize = CONFIG_NUM_DOMAINS * CONFIG_NUM_PRIORITIES;

/// kernel/include/object/structures.h
pub const TCB_SIZE_BITS: usize = seL4_TCBBits - 1;
pub const TCB_OFFSET: usize = BIT!(TCB_SIZE_BITS);
pub const tcbCTable: usize = 0;
pub const tcbVTable: usize = 1;
#[cfg(feature = "KERNEL_MCS")]
pub const tcbBuffer: usize = 2;
#[cfg(feature = "KERNEL_MCS")]
pub const tcbFaultHandler: usize = 3;
#[cfg(feature = "KERNEL_MCS")]
pub const tcbTimeoutHandler: usize = 4;
#[cfg(not(feature = "KERNEL_MCS"))]
pub const tcbReply: usize = 2;
#[cfg(not(feature = "KERNEL_MCS"))]
pub const tcbCaller: usize = 3;
#[cfg(not(feature = "KERNEL_MCS"))]
pub const tcbBuffer: usize = 4;
pub const tcbCNodeEntries: usize = 5;
pub const asidLowBits: usize = 9;
pub const asidHighBits: usize = 7;

/// kernel/libsel4/include/sel4/errors.h
pub const seL4_NoError: usize = 0;
pub const seL4_InvalidArgument: usize = 1;
pub const seL4_InvalidCapability: usize = 2;
pub const seL4_IllegalOperation: usize = 3;
pub const seL4_RangeError: usize = 4;
pub const seL4_AlignmentError: usize = 5;
pub const seL4_FailedLookup: usize = 6;
pub const seL4_TruncatedMessage: usize = 7;
pub const seL4_DeleteFirst: usize = 8;
pub const seL4_RevokeFirst: usize = 9;
pub const seL4_NotEnoughMemory: usize = 10;
pub const seL4_NumErrors: usize = 11;

/// kernel/libsel4/include/sel4/constants.h
/// enum seL4_MsgLimits {
/// seL4_MsgLengthBits = 7,
/// seL4_MsgExtraCapBits = 2,
/// seL4_MsgMaxLength = 120
/// };
pub const seL4_MsgLengthBits: usize = 7;
pub const seL4_MsgMaxLength: usize = 120;
pub const seL4_MsgExtraCapBits: usize = 2;
pub const seL4_MsgMaxExtraCaps: usize = BIT!(seL4_MsgExtraCapBits) - 1;
#[cfg(feature = "KERNEL_MCS")]
pub const seL4_MinSchedContextBits: usize = 7;
pub const seL4_MaxPrio: usize = CONFIG_NUM_PRIORITIES - 1;
pub const seL4_MinPrio: usize = 0;

/// kernel/libsel4/include/sel4/bootinfo_types.h
pub const SEL4_BOOTINFO_HEADER_FDT: usize = 6;
pub const SEL4_BOOTINFO_HEADER_PADDING: usize = 0;
pub const seL4_CapNull: usize = 0;
pub const seL4_CapInitThreadTCB: usize = 1;
pub const seL4_CapInitThreadCNode: usize = 2;
pub const seL4_CapInitThreadVspace: usize = 3;
pub const seL4_CapIRQControl: usize = 4;
pub const seL4_CapASIDControl: usize = 5;
pub const seL4_CapInitThreadASIDPool: usize = 6;
pub const seL4_CapIOPortControl: usize = 7;
pub const seL4_CapIOSpace: usize = 8;
pub const seL4_CapBootInfoFrame: usize = 9;
pub const seL4_CapInitThreadIPCBuffer: usize = 10;
pub const seL4_CapDomain: usize = 11;
pub const seL4_CapSMMUSIDControl: usize = 12;
pub const seL4_CapSMMUCBControl: usize = 13;
pub const seL4_CapInitThreadSC: usize = 14;
pub const seL4_CapSMC: usize = 15;
pub const seL4_NumInitialCaps: usize = 16;

/// kernel/include/machine/registerset.h
pub const MessageID_Syscall: usize = 0;
pub const MessageID_Exception: usize = 1;
#[cfg(feature = "KERNEL_MCS")]
pub const MessageID_TimeoutReply: usize = 2;

/// kernel/libsel4/sel4_arch_include/{arch}/sel4/sel4_arch/constants.h
pub const seL4_IPCBufferSizeBits: usize = 10;
pub const seL4_NumASIDPoolsBits: usize = 7;
pub const seL4_ASIDPoolIndexBits: usize = 9;
pub const seL4_ASIDPoolBits: usize = 12;
#[cfg(all(target_arch = "riscv64", feature = "HAVE_FPU"))]
pub const seL4_TCBBits: usize = 10;
#[cfg(all(target_arch = "riscv64", not(feature = "HAVE_FPU")))]
pub const seL4_TCBBits: usize = 11;
#[cfg(any(target_arch = "aarch64", test))]
pub const seL4_TCBBits: usize = 11;
pub const seL4_EndpointBits: usize = 4;
#[cfg(feature = "KERNEL_MCS")]
pub const seL4_NotificationBits: usize = 6;
#[cfg(feature = "KERNEL_MCS")]
pub const seL4_ReplyBits: usize = 5;
#[cfg(not(feature = "KERNEL_MCS"))]
pub const seL4_NotificationBits: usize = 5;
pub const seL4_SlotBits: usize = 5;
pub const seL4_MinUntypedBits: usize = 4;
pub const seL4_PageBits: usize = 12;
pub const seL4_PageTableBits: usize = 12;
pub const seL4_PageDirBits: usize = 12;
pub const seL4_PUDBits: usize = 12;
pub const seL4_PGDBits: usize = 12;
pub const seL4_HugePageBits: usize = 30;
pub const seL4_LargePageBits: usize = 21;
pub const seL4_PML4Bits: usize = 12;
pub const seL4_VSpaceBits: usize = seL4_PML4Bits;
pub const seL4_WordBits: usize = 64;
pub const seL4_UserTop: usize = 0x00007fffffffffff;
pub const USER_TOP: usize = seL4_UserTop;

/// rel4_kernel/sel4_common/src/sel4_config.rs
pub const ID_AA64PFR0_EL1_FP: u32 = 16;
pub const ID_AA64PFR0_EL1_ASIMD: u32 = 20;

/// kernel/include/api/types.h
pub const minDom: usize = 0;
pub const maxDom: usize = CONFIG_NUM_DOMAINS - 1;
pub const numDomains: usize = CONFIG_NUM_DOMAINS;

/// kernel/include/api/syscall.h
pub const TIME_ARG_SIZE: usize = 1;

/// kernel/include/arch/arm/arch/object/smc.h
#[cfg(feature = "ENABLE_SMC")]
pub const NUM_SMC_REGS: usize = 8;

/// kernel/include/arch/{arch}/arch/bootinfo.h
// TODO: generated by configure
pub const MAX_NUM_FREEMEM_REG: usize = 16;
pub const NUM_RESERVED_REGIONS: usize = 3;
pub const MAX_NUM_RESV_REG: usize = MAX_NUM_FREEMEM_REG + NUM_RESERVED_REGIONS;

/// kernel/include/arch/riscv/arch/machine.h
pub const SIP_SSIP: usize = 1;
pub const SIP_MSIP: usize = 3;
pub const SIP_STIP: usize = 5;
pub const SIP_MTIP: usize = 7;
pub const SIP_SEIP: usize = 9;
pub const SIP_MEIP: usize = 11;
pub const SIE_SSIE: usize = 1;
pub const SIE_MSIE: usize = 3;
pub const SIE_STIE: usize = 5;
pub const SIE_MTIE: usize = 7;
pub const SIE_SEIE: usize = 9;
pub const SIE_MEIE: usize = 11;

/// kernel/include/object/tcb.h
/// enum thread_control_caps_flag
/// {
///     thread_control_caps_update_ipc_buffer = 0x1,
///     thread_control_caps_update_space = 0x2,
///     thread_control_caps_update_fault = 0x4,
///     thread_control_caps_update_timeout = 0x8,
/// };
pub const thread_control_caps_update_ipc_buffer: usize = 0x1;
pub const thread_control_caps_update_space: usize = 0x2;
pub const thread_control_caps_update_fault: usize = 0x4;
pub const thread_control_caps_update_timeout: usize = 0x8;

/// enum thread_control_sched_flag
/// {
///     thread_control_sched_update_priority = 0x1,
///     thread_control_sched_update_mcp = 0x2,
///     thread_control_sched_update_sc = 0x4,
///     thread_control_sched_update_fault = 0x8,
/// };
pub const thread_control_sched_update_priority: usize = 0x1;
pub const thread_control_sched_update_mcp: usize = 0x2;
pub const thread_control_sched_update_sc: usize = 0x4;
pub const thread_control_sched_update_fault: usize = 0x8;

/// enum thread_control_flag
/// {
///     thread_control_update_priority = 0x1,
///     thread_control_update_ipc_buffer = 0x2,
///     thread_control_update_space = 0x4,
///     thread_control_update_mcp = 0x8,
/// #ifdef CONFIG_KERNEL_MCS
///     thread_control_update_sc = 0x10,
///     thread_control_update_fault = 0x20,
///     thread_control_update_timeout = 0x40,
/// #endif
/// };
pub const thread_control_update_priority: usize = 0x1;
pub const thread_control_update_ipc_buffer: usize = 0x2;
pub const thread_control_update_space: usize = 0x4;
pub const thread_control_update_mcp: usize = 0x8;
pub const thread_control_update_sc: usize = 0x10;
pub const thread_control_update_fault: usize = 0x20;
pub const thread_control_update_timeout: usize = 0x40;

/// kernel/include/bootinfo.h
pub const BI_FRAME_SIZE_BITS: usize = PAGE_BITS; 
pub const PAGE_SIZE_BITS: usize = PAGE_BITS;