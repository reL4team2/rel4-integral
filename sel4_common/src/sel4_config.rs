//! This module contains the configuration settings for sel4_common.
// include generated config constants
include!(concat!(env!("OUT_DIR"), "/config.rs"));

/// kernel/include/stdint.h
pub const UINT64_MAX: usize = 0xFFFFFFFFFFFFFFFF;

/// kernel/include/arch/{arch}/arch/64/mode/types.h
pub const WORD_RADIX: usize = 6;
pub const WORD_BITS: usize = bit!(WORD_RADIX);

/// kernel/include/arch/riscv/arch/object/structures.h
pub const PT_SIZE_BITS: usize = 12;
pub const PT_INDEX_BITS: usize = 9;
pub const PD_INDEX_BITS: usize = 9;
pub const UPUD_INDEX_BITS: usize = 9;
pub const PUD_INDEX_BITS: usize = 9;
pub const PGD_INDEX_BITS: usize = 9;

/// kernel/include/arch/riscv/arch/machine/hardware.h
pub const PAGE_BITS: usize = SEL4_PAGE_BITS;
pub const RISCV_4K_PAGE: usize = 0;
pub const RISCV_MEGA_PAGE: usize = 1;
pub const RISCV_GIGA_PAGE: usize = 2;
pub const RISCV_TERA_PAGE: usize = 3;
pub const RISCV_PAGE_BITS: usize = SEL4_PAGE_BITS;
pub const RISCV_MEGA_PAGE_BITS: usize = SEL4_LARGE_PAGE_BITS;
pub const RISCV_GIGA_PAGE_BITS: usize = SEL4_HUGE_PAGE_BITS;
pub const RISCV_INSTRUCTION_MISALIGNED: usize = 0;
pub const RISCV_INSTRUCTION_ACCESS_FAULT: usize = 1;
pub const RISCV_INSTRUCTION_ILLEGAL: usize = 2;
pub const RISCV_BREAK_POINT: usize = 3;
pub const RISCV_LOAD_ACCESS_FAULT: usize = 5;
pub const RISCV_ADDRESS_MISALIGNED: usize = 6;
pub const RISCV_STORE_ACCESS_FAULT: usize = 7;
pub const RISCV_ENV_CALL: usize = 8;
pub const RISCV_INSTRUCTION_PAGE_FAULT: usize = 12;
pub const RISCV_LOAD_PAGE_FAULT: usize = 13;
pub const RISCV_STORE_PAGE_FAULT: usize = 15;

/// kernel/include/arch/arm/arch/64/mode/machine/hardware.h
/// enum vm_page_size {
///     ARMSmallPage,
///     ARMLargePage,
///     ARMHugePage
///};
pub const ARM_SMALL_PAGE: usize = 0;
pub const ARM_LARGE_PAGE: usize = 1;
pub const ARM_HUGE_PAGE: usize = 2;
pub const ARM_SMALL_PAGE_BITS: usize = SEL4_PAGE_BITS;
pub const ARM_LARGE_PAGE_BITS: usize = SEL4_LARGE_PAGE_BITS;
pub const ARM_HUGE_PAGE_BITS: usize = SEL4_HUGE_PAGE_BITS;

/// kernel/include/arch/{arch}/mode/object/structures.h
/// kernel/include/arch/arm/arch/64/mode/object/structures.h
pub const PT_INDEX_OFFSET: usize = SEL4_PAGE_BITS;
pub const PD_INDEX_OFFSET: usize = PT_INDEX_OFFSET + PT_INDEX_BITS;
pub const PUD_INDEX_OFFSET: usize = PD_INDEX_OFFSET + PD_INDEX_BITS;
pub const PGD_INDEX_OFFSET: usize = PUD_INDEX_OFFSET + PUD_INDEX_BITS;
pub const N_ASID_POOLS: usize = bit!(ASID_HIGH_BITS);
pub const ASID_BITS: usize = ASID_HIGH_BITS + ASID_LOW_BITS;

/// kernel/include/arch/{arch}/arch/api/types.h
pub const ASID_INVALID: usize = 0;

/// kernel/include/arch/{arch}/arch/kernel/vspace.h
pub const IT_ASID: usize = 1;

/// kernel/include/model
// pub const L2_BITMAP_SIZE: usize = (CONFIG_NUM_PRIORITIES + WORD_BITS - 1) / WORD_BITS;
pub const L2_BITMAP_SIZE: usize = CONFIG_NUM_PRIORITIES.div_ceil(WORD_BITS);
pub const NUM_READY_QUEUES: usize = CONFIG_NUM_DOMAINS * CONFIG_NUM_PRIORITIES;

/// kernel/include/object/structures.h
pub const TCB_SIZE_BITS: usize = SEL4_TCB_BITS - 1;
pub const TCB_OFFSET: usize = bit!(TCB_SIZE_BITS);
pub const TCB_CTABLE: usize = 0;
pub const TCB_VTABLE: usize = 1;
#[cfg(feature = "kernel_mcs")]
pub const TCB_BUFFER: usize = 2;
#[cfg(feature = "kernel_mcs")]
pub const TCB_FAULT_HANDLER: usize = 3;
#[cfg(feature = "kernel_mcs")]
pub const TCB_TIMEOUT_HANDLER: usize = 4;
#[cfg(not(feature = "kernel_mcs"))]
pub const TCB_REPLY: usize = 2;
#[cfg(not(feature = "kernel_mcs"))]
pub const TCB_CALLER: usize = 3;
#[cfg(not(feature = "kernel_mcs"))]
pub const TCB_BUFFER: usize = 4;
pub const TCB_CNODE_ENTRIES: usize = 5;
pub const ASID_LOW_BITS: usize = 9;
pub const ASID_HIGH_BITS: usize = 7;

/// kernel/libsel4/include/sel4/errors.h
pub const SEL4_NO_ERROR: usize = 0;
pub const SEL4_INVALID_ARGUMENT: usize = 1;
pub const SEL4_INVALID_CAPABILITY: usize = 2;
pub const SEL4_ILLEGAL_OPERATION: usize = 3;
pub const SEL4_RANGE_ERROR: usize = 4;
pub const SEL4_ALIGNMENT_ERROR: usize = 5;
pub const SEL4_FAILED_LOOKUP: usize = 6;
pub const SEL4_TRUNCATED_MESSAGE: usize = 7;
pub const SEL4_DELETE_FIRST: usize = 8;
pub const SEL4_REVOKE_FIRST: usize = 9;
pub const SEL4_NOT_ENOUGH_MEMORY: usize = 10;
pub const SEL4_NUM_ERRORS: usize = 11;

/// kernel/libsel4/include/sel4/constants.h
/// enum seL4_MsgLimits {
/// SEL4_MSG_LENGTH_BITS = 7,
/// SEL4_MSG_EXTRA_CAP_BITS = 2,
/// SEL4_MSG_MAX_LENGTH = 120
/// };
pub const SEL4_MSG_LENGTH_BITS: usize = 7;
pub const SEL4_MSG_MAX_LENGTH: usize = 120;
pub const SEL4_MSG_EXTRA_CAP_BITS: usize = 2;
pub const SEL4_MSG_MAX_EXTRA_CAPS: usize = bit!(SEL4_MSG_EXTRA_CAP_BITS) - 1;
#[cfg(feature = "kernel_mcs")]
pub const SEL4_MIN_SCHED_CONTEXT_BITS: usize = 7;
pub const SEL4_MAX_PRIO: usize = CONFIG_NUM_PRIORITIES - 1;
pub const SEL4_MIN_PRIO: usize = 0;

/// kernel/libsel4/include/sel4/bootinfo_types.h
pub const SEL4_BOOTINFO_HEADER_FDT: usize = 6;
pub const SEL4_BOOTINFO_HEADER_PADDING: usize = 0;
pub const SEL4_CAP_NULL: usize = 0;
pub const SEL4_CAP_INIT_THREAD_TCB: usize = 1;
pub const SEL4_CAP_INIT_THREAD_CNODE: usize = 2;
pub const SEL4_CAP_INIT_THREAD_VSPACE: usize = 3;
pub const SEL4_CAP_IRQ_CONTROL: usize = 4;
pub const SEL4_CAP_ASID_CONTROL: usize = 5;
pub const SEL4_CAP_INIT_THREAD_ASID_POOL: usize = 6;
pub const SEL4_CAP_IO_PORT_CONTROL: usize = 7;
pub const SEL4_CAP_IO_SPACE: usize = 8;
pub const SEL4_CAP_BOOT_INFO_FRAME: usize = 9;
pub const SEL4_CAP_INIT_THREAD_IPC_BUFFER: usize = 10;
pub const SEL4_CAP_DOMAIN: usize = 11;
pub const SEL4_CAP_SMMU_SID_CONTROL: usize = 12;
pub const SEL4_CAP_SMMU_CB_CONTROL: usize = 13;
pub const SEL4_CAP_INIT_THREAD_SC: usize = 14;
pub const SEL4_CAP_SMC: usize = 15;
pub const SEL4_NUM_INITIAL_CAPS: usize = 16;

/// kernel/include/machine/registerset.h
pub const MESSAGE_ID_SYSCALL: usize = 0;
pub const MESSAGE_ID_EXCEPTION: usize = 1;
#[cfg(feature = "kernel_mcs")]
pub const MESSAGE_ID_TIMEOUT_REPLY: usize = 2;

/// kernel/libsel4/sel4_arch_include/{arch}/sel4/sel4_arch/constants.h
pub const SEL4_IPC_BUFFER_SIZE_BITS: usize = 10;
pub const SEL4_NUM_ASID_POOLS_BITS: usize = 7;
pub const SEL4_ASID_POOL_INDEX_BITS: usize = 9;
pub const SEL4_ASID_POOL_BITS: usize = 12;
#[cfg(all(target_arch = "riscv64", feature = "have_fpu"))]
pub const SEL4_TCB_BITS: usize = 11;
#[cfg(all(target_arch = "riscv64", not(feature = "have_fpu")))]
pub const SEL4_TCB_BITS: usize = 10;
#[cfg(any(target_arch = "aarch64", test))]
pub const SEL4_TCB_BITS: usize = 11;
pub const SEL4_ENDPOINT_BITS: usize = 4;
#[cfg(feature = "kernel_mcs")]
pub const SEL4_NOTIFICATION_BITS: usize = 6;
#[cfg(feature = "kernel_mcs")]
pub const SEL4_REPLY_BITS: usize = 5;
#[cfg(not(feature = "kernel_mcs"))]
pub const SEL4_NOTIFICATION_BITS: usize = 5;
pub const SEL4_SLOT_BITS: usize = 5;
pub const SEL4_MIN_UNTYPED_BITS: usize = 4;
pub const SEL4_PAGE_BITS: usize = 12;
pub const SEL4_PAGE_TABLE_BITS: usize = 12;
pub const SEL4_PAGE_DIR_BITS: usize = 12;
pub const SEL4_PUD_BITS: usize = 12;
pub const SEL4_PGD_BITS: usize = 12;
pub const SEL4_HUGE_PAGE_BITS: usize = 30;
pub const SEL4_LARGE_PAGE_BITS: usize = 21;
pub const SEL4_PML4_BITS: usize = 12;
pub const SEL4_VSPACE_BITS: usize = SEL4_PML4_BITS;
pub const SEL4_WORD_BITS: usize = 64;
pub const SEL4_USER_TOP: usize = 0x00007fffffffffff;
pub const USER_TOP: usize = SEL4_USER_TOP;

/// rel4_kernel/sel4_common/src/sel4_config.rs
pub const ID_AA64PFR0_EL1_FP: u32 = 16;
pub const ID_AA64PFR0_EL1_ASIMD: u32 = 20;

/// kernel/include/api/types.h
pub const MIN_DOM: usize = 0;
pub const MAX_DOM: usize = CONFIG_NUM_DOMAINS - 1;
pub const NUM_DOMAINS: usize = CONFIG_NUM_DOMAINS;

/// kernel/include/api/syscall.h
pub const TIME_ARG_SIZE: usize = 1;

/// kernel/include/arch/arm/arch/object/smc.h
#[cfg(feature = "enable_smc")]
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
///     THREAD_CONTROL_CAPS_UPDATE_IPC_BUFFER = 0x1,
///     THREAD_CONTROL_CAPS_UPDATE_SPACE = 0x2,
///     THREAD_CONTROL_CAPS_UPDATE_FAULT = 0x4,
///     THREAD_CONTROL_CAPS_UPDATE_TIMEOUT = 0x8,
/// };
pub const THREAD_CONTROL_CAPS_UPDATE_IPC_BUFFER: usize = 0x1;
pub const THREAD_CONTROL_CAPS_UPDATE_SPACE: usize = 0x2;
pub const THREAD_CONTROL_CAPS_UPDATE_FAULT: usize = 0x4;
pub const THREAD_CONTROL_CAPS_UPDATE_TIMEOUT: usize = 0x8;

/// enum thread_control_sched_flag
/// {
///     THREAD_CONTROL_SCHED_UPDATE_PRIORITY = 0x1,
///     THREAD_CONTROL_SCHED_UPDATE_MCP = 0x2,
///     THREAD_CONTROL_SCHED_UPDATE_SC = 0x4,
///     THREAD_CONTROL_SCHED_UPDATE_FAULT = 0x8,
/// };
pub const THREAD_CONTROL_SCHED_UPDATE_PRIORITY: usize = 0x1;
pub const THREAD_CONTROL_SCHED_UPDATE_MCP: usize = 0x2;
pub const THREAD_CONTROL_SCHED_UPDATE_SC: usize = 0x4;
pub const THREAD_CONTROL_SCHED_UPDATE_FAULT: usize = 0x8;

/// enum thread_control_flag
/// {
///     THREAD_CONTROL_UPDATE_PRIORITY = 0x1,
///     THREAD_CONTROL_UPDATE_IPC_BUFFER = 0x2,
///     THREAD_CONTROL_UPDATE_SPACE = 0x4,
///     THREAD_CONTROL_UPDATE_MCP = 0x8,
/// #ifdef CONFIG_KERNEL_MCS
///     THREAD_CONTROL_UPDATE_SC = 0x10,
///     THREAD_CONTROL_UPDATE_FAULT = 0x20,
///     THREAD_CONTROL_UPDATE_TIMEOUT = 0x40,
/// #endif
/// };
pub const THREAD_CONTROL_UPDATE_PRIORITY: usize = 0x1;
pub const THREAD_CONTROL_UPDATE_IPC_BUFFER: usize = 0x2;
pub const THREAD_CONTROL_UPDATE_SPACE: usize = 0x4;
pub const THREAD_CONTROL_UPDATE_MCP: usize = 0x8;
pub const THREAD_CONTROL_UPDATE_SC: usize = 0x10;
pub const THREAD_CONTROL_UPDATE_FAULT: usize = 0x20;
pub const THREAD_CONTROL_UPDATE_TIMEOUT: usize = 0x40;

/// kernel/include/bootinfo.h
pub const BI_FRAME_SIZE_BITS: usize = PAGE_BITS;
pub const PAGE_SIZE_BITS: usize = PAGE_BITS;
