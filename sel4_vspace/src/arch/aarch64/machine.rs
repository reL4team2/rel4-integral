use core::arch::asm;

use aarch64_cpu::registers::Writeable;
use aarch64_cpu::{asm::barrier, registers};
use rel4_arch::basic::PAddr;
use sel4_common::sel4_config::CONFIG_L1_CACHE_LINE_SIZE_BITS;
#[inline]
pub fn set_current_kernel_vspace_root(val: usize) {
    #[cfg(not(feature = "hypervisor"))]
    {
        registers::TTBR1_EL1.set(val as _);
    }
    #[cfg(feature = "hypervisor")]
    {
        registers::TTBR0_EL2.set(val as _);
        unsafe { core::arch::asm!("TLBI ALLE2") };
    }
    barrier::dsb(barrier::SY);
    barrier::isb(barrier::SY);
}

#[inline]
pub fn set_current_user_vspace_root(val: usize) {
    #[cfg(not(feature = "hypervisor"))]
    {
        registers::TTBR0_EL1.set(val as _);
        unsafe { core::arch::asm!("tlbi vmalle1") };
    }
    #[cfg(feature = "hypervisor")]
    {
        registers::VTTBR_EL2.set(val as _);
        invalidate_local_tlb();
        unsafe {
            asm!("tlbi alle2");
            dsb();
            asm!("tlbi alle1");
        }
    }
    dsb();
    isb();
    // log::warn!("virtual ttbr el2: {:#x}", val);
    // FIXME: use aisd instead of flush tlb
}

#[inline]
pub const fn ttbr_new(asid: usize, addr: PAddr) -> usize {
    (asid & 0xffff) << 48 | (addr.raw() & 0xffffffffffff)
}

/**
 * sy（System）: 确保所有CPU都看到之前的存储操作的效果，这是最常用的级别，提供全系统范围的数据同步。
 * st（Store）: 确保之前的所有存储操作对其他处理器可见，主要用于控制存储操作的完成。
 * ld（Load）: 确保之前的所有加载操作完成，主要用于加载操作。
 * ish（Inner Shareable）: 仅确保同一内存共享域内的处理器看到之前的存储操作的效果。
 * ishst（Inner Shareable for Stores）: 类似于ish，但仅适用于存储操作。
 * nsh（Non-shareable）: 仅在非共享内存区域内确保之前的操作完成。
 * nshst（Non-shareable for Stores）: 类似于nsh，但仅适用于存储操作。
 * osh（Outer Shareable）: 确保操作对外部共享内存域内的所有处理器可见。
 * oshst（Outer Shareable for Stores）: 类似于osh，但仅适用于存储操作。
*/
#[inline]
pub fn dsb() {
    barrier::dsb(barrier::SY);
}

#[inline]
pub fn isb() {
    barrier::isb(barrier::SY);
}

#[inline]
pub fn invalidate_local_tlb_asid(asid: usize) {
    assert!(asid < (1 << 16)); // BIT(16) 相当于 1 << 16

    dsb();
    unsafe {
        asm!("tlbi aside1, {}", in(reg) (asid << 48));
    }
    dsb();
    isb();
}

#[inline]
pub fn invalidate_local_tlb_va_asid(mva_plus_asid: usize) {
    dsb();
    unsafe {
        asm!("tlbi vae1, {}", in(reg) mva_plus_asid);
    }
    dsb();
    isb();
}

#[inline(always)]
pub fn clean_by_va_pou(vaddr: usize, _paddr: PAddr) {
    unsafe {
        asm!("dc cvau, {}", in(reg) vaddr);
    }
    dmb();
}

#[inline(always)]
pub fn clean_by_va(vaddr: usize, _paddr: PAddr) {
    unsafe {
        asm!("dc cvac, {}", in(reg) vaddr);
    }
    dmb();
}

#[inline(always)]
pub fn invalidate_by_va(vaddr: usize, _paddr: PAddr) {
    unsafe {
        asm!("dc ivac, {}", in(reg) vaddr);
    }
    dmb();
}

#[inline(always)]
pub fn clean_inval_by_va(vaddr: usize, _paddr: PAddr) {
    unsafe {
        asm!("dc civac, {}", in(reg) vaddr);
    }
    dsb();
}

#[inline(always)]
pub fn invalidate_by_va_i(vaddr: usize, _paddr: PAddr) {
    unsafe {
        asm!("ic ivau, {}", in(reg) vaddr);
    }
    dsb();
    isb();
}

#[inline(always)]
pub fn dmb() {
    unsafe {
        asm!("dmb sy", options(nostack, preserves_flags));
    }
}

// TIPS: please use const to make code cleaner and faster.

pub fn clean_cache_range_ram(start: usize, end: usize, pstart: PAddr) {
    clean_cache_range_poc(start, end, pstart);

    dsb();

    plat_clean_l2_range(pstart, pstart + (end - start));
}

#[inline]
const fn LINE_START(a: usize) -> usize {
    round_down!(a, CONFIG_L1_CACHE_LINE_SIZE_BITS)
}

#[inline]
const fn LINE_INDEX(a: usize) -> usize {
    LINE_START(a) >> CONFIG_L1_CACHE_LINE_SIZE_BITS
}

#[inline]
pub fn invalidate_cache_range_i(start: usize, end: usize, pstart: PAddr) {
    for idx in LINE_INDEX(start)..LINE_INDEX(end) + 1 {
        let line = idx << CONFIG_L1_CACHE_LINE_SIZE_BITS;
        invalidate_by_va_i(line, pstart + line - start);
    }
}

#[inline]
pub fn clean_cache_range_poc(start: usize, end: usize, pstart: PAddr) {
    for idx in LINE_INDEX(start)..LINE_INDEX(end) + 1 {
        let line = idx << CONFIG_L1_CACHE_LINE_SIZE_BITS;
        clean_by_va(line, pstart + line - start);
    }
}

#[inline]
pub fn clean_cache_range_pou(start: usize, end: usize, pstart: PAddr) {
    for idx in LINE_INDEX(start)..LINE_INDEX(end) + 1 {
        let line = idx << CONFIG_L1_CACHE_LINE_SIZE_BITS;
        clean_by_va_pou(line, pstart + line - start);
    }
}

pub fn plat_clean_l2_range(_pstart: PAddr, _pend: PAddr) {}

#[inline]
const fn loc(x: usize) -> usize {
    (x >> 24) & mask_bits!(3)
}

#[inline]
const fn ctype(x: usize, n: usize) -> usize {
    (x >> (n * 3)) & mask_bits!(3)
}

#[inline]
const fn line_bits(s: usize) -> usize {
    (s & mask_bits!(3)) + 4
}

#[inline]
const fn assoc(s: usize) -> usize {
    ((s >> 3) & mask_bits!(10)) + 1
}

#[inline]
const fn nsets(s: usize) -> usize {
    ((s >> 13) & mask_bits!(15)) + 1
}

pub enum arm_cache_type {
    ARMCacheI = 1,
    ARMCacheD = 2,
    ARMCacheID = 3,
}

fn plat_cleanInvalidateL2Range(_start: usize, _end: usize) {}

#[inline]
pub fn clean_invalidate_cache_range_ram(start: usize, end: usize, pstart: PAddr) {
    clean_cache_range_poc(start, end, pstart);

    dsb();

    plat_cleanInvalidateL2Range(pstart.raw(), pstart.raw() + end - start);
    for idx in LINE_INDEX(start)..LINE_INDEX(end) + 1 {
        let line = idx << CONFIG_L1_CACHE_LINE_SIZE_BITS;
        clean_inval_by_va(line, pstart + line - start);
    }
    dsb();
}

fn plat_invalidateL2Range(_start: usize, _end: usize) {}

#[inline]
pub fn invalidate_cache_range_ram(start: usize, end: usize, pstart: PAddr) {
    if start != LINE_START(start) {
        clean_cache_range_ram(start, end, pstart);
    }
    if end + 1 != LINE_START(end + 1) {
        let line = LINE_START(end);
        clean_cache_range_ram(line, line, pstart + line - start);
    }
    plat_invalidateL2Range(pstart.raw(), pstart.raw() + end - start);

    for idx in LINE_INDEX(start)..LINE_INDEX(end) + 1 {
        let line = idx << CONFIG_L1_CACHE_LINE_SIZE_BITS;
        invalidate_by_va(line, pstart + line - start);
    }
    dsb();
}

pub fn clean_invalidate_l1_caches() {
    dsb();
    clean_invalidate_d_poc();
    dsb();
    invalidate_i_pou();
    dsb();
}

#[inline]
pub fn invalidate_i_pou() {
    unsafe {
        asm!("ic iallu");
    }
    isb();
}

pub fn clean_invalidate_d_poc() {
    let clid = read_clid();
    let loc = loc(clid);

    for l in 0..loc {
        if ctype(clid, l) > arm_cache_type::ARMCacheI as usize {
            clean_invalidate_d_by_level(l);
        }
    }
}

#[inline]
fn clean_invalidate_d_by_level(l: usize) {
    let lsize = read_cache_size(l, 0);
    let lbits = line_bits(lsize);
    let assoc = assoc(lsize);
    let assoc_bits = 64 - (assoc - 1).leading_zeros() as usize;
    let nsets = nsets(lsize);

    for w in 0..assoc {
        for s in 0..nsets {
            clean_invalidate_by_wsl((w << (32 - assoc_bits)) | (s << lbits) | (l << 1));
        }
    }
}

#[inline]
fn clean_invalidate_by_wsl(wsl: usize) {
    unsafe {
        asm!("dc cisw, {}", in(reg) wsl);
    }
}

#[inline]
fn read_cache_size(level: usize, instruction: usize) -> usize {
    let size: usize;
    let csselr_old: usize;
    unsafe {
        // save CSSELR
        asm!("mrs {}, csselr_el1", out(reg) csselr_old);
        // select cache level
        asm!("msr csselr_el1, {}", in(reg) ((level << 1) | instruction));
        // read 'size'
        asm!("mrs {}, ccsidr_el1", out(reg) size);
        // restore CSSELR
        asm!("msr csselr_el1, {}", in(reg) csselr_old);
    }
    size
}

#[inline]
fn read_clid() -> usize {
    let clid: usize;
    unsafe {
        asm!("mrs {}, clidr_el1", out(reg) clid);
    }
    clid
}

#[inline]
pub fn invalidate_local_tlb() {
    dsb();
    unsafe {
        asm!("tlbi vmalle1");
    }
    dsb();
    isb();
}

/*
 * Memory types are defined in Memory Attribute Indirection Register.
 *  - nGnRnE Device non-Gathering, non-Reordering, No Early write acknowledgement
 *  - nGnRE Unused Device non-Gathering, non-Reordering, Early write acknowledgement
 *  - GRE Unused Device Gathering, Reordering, Early write acknowledgement
 *  - NORMAL_NC Normal Memory, Inner/Outer non-cacheable
 *  - NORMAL Normal Memory, Inner/Outer Write-back non-transient, Write-allocate, Read-allocate
 *  - NORMAL_WT Normal Memory, Inner/Outer Write-through non-transient, No-Write-allocate, Read-allocate
 * Note: These should match with contents of MAIR_EL1 register!
 */
pub enum mair_types {
    DEVICE_nGnRnE,
    DEVICE_nGnRE,
    DEVICE_GRE,
    NORMAL_NC,
    NORMAL,
    NORMAL_WT,
}
