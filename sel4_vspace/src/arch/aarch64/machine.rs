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

#[cfg(feature = "hypervisor")]
#[inline]
pub fn invalidate_local_tlb_vmid(vmid: usize) {
    use aarch64_cpu::registers::{Readable, Writeable, VTTBR_EL2};

    let vttbr = VTTBR_EL2.get();
    let v = ((vttbr >> 48) & 0xffff) as usize;
    dsb();
    // tlbi vmalls12e1 only affects the VMID currently in VTTBR_EL2, so switch
    // to the target VMID first (matches C kernel invalidateLocalTLB_VMID).
    if v != vmid {
        VTTBR_EL2.set((vmid as u64) << 48);
        dsb();
        isb();
    }
    unsafe { asm!("tlbi vmalls12e1") };
    dsb();
    isb();
    if v != vmid {
        VTTBR_EL2.set(vttbr);
        dsb();
        isb();
    }
}

#[cfg(feature = "hypervisor")]
#[inline]
pub fn invalidate_local_tlb_ipa_vmid(ipa_plus_vmid: usize) {
    use aarch64_cpu::registers::{Readable, Writeable, VTTBR_EL2};

    let vttbr = VTTBR_EL2.get();
    let v = ((vttbr >> 48) & 0xffff) as usize;
    let vmid = (ipa_plus_vmid >> 48) & 0xffff;
    // C kernel: "The [0:35] bits are IPA, other bits are reserved as 0".
    let ipa = ipa_plus_vmid & 0xfffffffff;
    dsb();
    if v != vmid {
        VTTBR_EL2.set((vmid as u64) << 48);
        dsb();
        isb();
    }
    unsafe {
        asm!("tlbi ipas2e1, {}", in(reg) ipa);
        dsb();
        asm!("tlbi vmalle1");
    }
    dsb();
    isb();
    if v != vmid {
        VTTBR_EL2.set(vttbr);
        dsb();
        isb();
    }
}

#[inline(always)]
pub fn invalidate_local_tlb_va_asid(mva_plus_asid: usize) {
    unsafe {
        core::arch::asm!(
            "dsb sy",
            "tlbi vae1, {}",
            "dsb sy",
            "isb",
            in(reg) mva_plus_asid,
        );
    }
}

#[inline(always)]
pub fn clean_by_va_pou(vaddr: usize, _paddr: PAddr) {
    // Matches C kernel cleanByVA_PoU: dc cvau (clean to PoU) + dmb.
    // The C kernel uses this for page-table writes in both stage-1 and
    // stage-2 (hypervisor) cases.
    unsafe {
        core::arch::asm!(
            "dc cvau, {}",
            in(reg) vaddr,
        );
    }
    dmb();
}

#[inline(always)]
pub fn clean_by_va(vaddr: usize, _paddr: PAddr) {
    unsafe {
        asm!("dc cvac, {}", in(reg) vaddr);
    }
    // Matches C kernel cleanByVA (dmb). The enclosing clean*Range_RAM does dsb().
    dmb();
}

#[inline(always)]
pub fn invalidate_by_va(vaddr: usize, _paddr: PAddr) {
    unsafe {
        asm!("dc ivac, {}", in(reg) vaddr);
    }
    // Matches C kernel invalidateByVA (dmb).
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
    if end <= start {
        return;
    }
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
    if start == 0 || end <= start {
        return;
    }
    #[cfg(feature = "hypervisor")]
    {
        // A72 L1 I-cache is VIPT. In hypervisor mode the VA passed here is a
        // kernel alias for the underlying physical memory, which cannot correctly
        // index a VIPT I-cache. Invalidate the entire I-cache instead (matches
        // C kernel invalidateCacheRange_I under ICACHE_VIPT + hypervisor).
        let _ = (start, end, pstart);
        invalidate_i_pou();
    }
    #[cfg(not(feature = "hypervisor"))]
    {
        for idx in LINE_INDEX(start)..LINE_INDEX(end) + 1 {
            let line = idx << CONFIG_L1_CACHE_LINE_SIZE_BITS;
            invalidate_by_va_i(line, pstart + line - start);
        }
    }
}

#[inline]
pub fn clean_cache_range_poc(start: usize, end: usize, pstart: PAddr) {
    if end <= start {
        return;
    }
    for idx in LINE_INDEX(start)..LINE_INDEX(end) + 1 {
        let line = idx << CONFIG_L1_CACHE_LINE_SIZE_BITS;
        clean_by_va(line, pstart + line - start);
    }
}

#[inline]
pub fn clean_cache_range_pou(start: usize, end: usize, pstart: PAddr) {
    if end <= start {
        return;
    }
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
    if end <= start {
        return;
    }
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
    if end <= start {
        return;
    }
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
/// Perform Stage-1 address translation for an IPA when VCPU is active.
/// Uses AT S1E1R to walk the VM's Stage-1 page tables and returns
/// the resulting physical address from PAR_EL1.
/// Returns the full PAR_EL1 value (contains fault info if translation fails).
#[cfg(feature = "hypervisor")]
#[inline]
pub fn address_translate_s1(vaddr: usize) -> usize {
    unsafe {
        // AT S1E1R: Address Translate Stage 1 EL1 Read
        core::arch::asm!("at s1e1r, {}", in(reg) vaddr);
    }
    barrier::isb(barrier::SY);
    let par: usize;
    unsafe {
        core::arch::asm!("mrs {}, par_el1", out(reg) par);
    }
    par
}

/// Extract the output address from a PAR_EL1 value.
/// PAR_EL1[47:12] contains the output address bits for successful translations.
#[cfg(feature = "hypervisor")]
#[inline]
pub const fn get_par_addr(par: usize) -> usize {
    par & 0x0000_ffff_ffff_f000
}

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
 *
 * Stage-2 translation memory attributes (used when CONFIG_ARM_HYPERVISOR_SUPPORT is enabled).
 * These correspond to MAIR_EL2 attribute indices for VTTBR_EL2 page tables.
 */
pub enum mair_types {
    DEVICE_nGnRnE = 0,
    DEVICE_nGnRE = 1,
    DEVICE_GRE = 2,
    NORMAL_NC = 3,
    NORMAL = 4,
    NORMAL_WT = 5,

    // Stage-2 normal memory attribute index. MAIR_EL2 holds 16 attributes
    // (Attr0-Attr15) of 4 bits each; index 15 = Normal Inner/Outer WB-WA-RA.
    S2_NORMAL = 15,
}
