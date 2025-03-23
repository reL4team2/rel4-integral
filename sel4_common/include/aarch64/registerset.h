/*
 * Copyright 2020, Data61, CSIRO (ABN 41 687 119 230)
 *
 * SPDX-License-Identifier: GPL-2.0-only
 */

#pragma once

/* Control register fields */
#define CONTROL_M   0  /* MMU enable */
#define CONTROL_A   1  /* Alignment fault enable */
#define CONTROL_C   2  /* L1 data cache enable */
#define CONTROL_W   3  /* Write buffer enable */
#define CONTROL_B   7  /* Big endian mode */
#define CONTROL_S   8  /* System protection (deprecated) */
#define CONTROL_R   9  /* ROM protection (deprecated) */
#define CONTROL_Z   11 /* Flow prediction enable */
#define CONTROL_I   12 /* L1 instruction cache enable */
#define CONTROL_V   13 /* Exception vector remap */
#define CONTROL_RR  14 /* Cache replacement strategy */
#define CONTROL_FI  21 /* Fast Interrupt enable */
#define CONTROL_U   22 /* Unaligned access enable */
#define CONTROL_XP  23 /* Subpage AP bits disable */
#define CONTROL_VE  24 /* Vectored interrupt enable */
#define CONTROL_EE  25 /* Exception E bit */
#define CONTROL_TRE 28 /* TEX remap enable */
#define CONTROL_AP  29 /* Access Flag Enable */

#define CONTROL_M         0  /* MMU enable */
#define CONTROL_A         1  /* Alignment check enable */
#define CONTROL_C         2  /* Cacheability control, for data caching */
#define CONTROL_SA0       4  /* Stack Alignment Check Enable for EL0 */
#define CONTROL_SA        3  /* Stack Alignment Check for EL1 */
#define CONTROL_I         12 /* Instruction access Cacheability control */
#define CONTROL_E0E       24 /* Endianness of data accesses at EL0 */
#define CONTROL_EE        25 /* Endianness of data accesses at EL1 */

/* CurrentEL register */
#define PEXPL1                  (1 << 2)
#define PEXPL2                  (1 << 3)

/* PSTATE register */
#define PMODE_FIRQ              (1 << 6)
#define PMODE_IRQ               (1 << 7)
#define PMODE_SERROR            (1 << 8)
#define PMODE_DEBUG             (1 << 9)
#define PMODE_EL0t              0
#define PMODE_EL1t              4
#define PMODE_EL1h              5
#define PMODE_EL2h              9

/* DAIF register */
#define DAIF_FIRQ               (1 << 6)
#define DAIF_IRQ                (1 << 7)
#define DAIF_SERROR             (1 << 8)
#define DAIF_DEBUG              (1 << 9)
#define DAIFSET_MASK            0xf

/* ESR register */
#define ESR_EC_SHIFT            26
#define ESR_EC_LEL_DABT         0x24    // Data abort from a lower EL
#define ESR_EC_CEL_DABT         0x25    // Data abort from the current EL
#define ESR_EC_LEL_IABT         0x20    // Instruction abort from a lower EL
#define ESR_EC_CEL_IABT         0x21    // Instruction abort from the current EL
#define ESR_EC_LEL_SVC64        0x15    // SVC from a lower EL in AArch64 state
#define ESR_EC_LEL_HVC64        0x16    // HVC from EL1 in AArch64 state
#define ESR_EL1_EC_ENFP         0x7     // Access to Advanced SIMD or floating-point registers


/* ID_AA64PFR0_EL1 register */
#define ID_AA64PFR0_EL1_FP      16     // HWCap for Floating Point
#define ID_AA64PFR0_EL1_ASIMD   20     // HWCap for Advanced SIMD

/* CPACR_EL1 register */
#define CPACR_EL1_FPEN          20     // FP regiters access

/* Offsets within the user context, these need to match the order in
 * register_t below */
#define PT_LR                       (30 * 8)
#define PT_SP_EL0                   (31 * 8)
#define PT_ELR_EL1                  (32 * 8)
#define PT_SPSR_EL1                 (33 * 8)
#define PT_FaultIP                  (34 * 8)
#define PT_TPIDR_EL0                (35 * 8)
