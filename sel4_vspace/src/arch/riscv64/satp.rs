use rel4_arch::basic::PAddr;
use riscv::register::satp;
#[cfg(feature = "enable_smp")]
use sel4_common::arch::riscv64::remote_sfence_vma;

///`satp`寄存器对应的内存备份
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct satp_t {
    pub words: usize,
}

impl satp_t {
    pub fn new(mode: usize, asid: usize, ppn: usize) -> Self {
        satp_t {
            words: 0
                | (mode & 0xfusize) << 60
                | (asid & 0xffffusize) << 44
                | (ppn & 0xfffffffffffusize) << 0,
        }
    }
}

#[cfg(feature = "enable_smp")]
#[inline]
pub fn sfence() {
    use sel4_common::arch::get_sbi_mask_for_all_remote_harts;

    unsafe {
        core::arch::asm!("fence w, rw");
    }
    sfence_local();
    let mask = get_sbi_mask_for_all_remote_harts();
    remote_sfence_vma(mask, 0, 0);
}

#[cfg(feature = "enable_smp")]
#[inline]
pub fn sfence_local() {
    unsafe {
        core::arch::asm!("sfence.vma");
    }
}

///对汇编指令`sfence.vma`的简单封装，清空`cache`、`tlb`
///
/// Risc-v's sfence.vma
#[cfg(not(feature = "enable_smp"))]
#[inline]
pub fn sfence() {
    #[cfg(target_arch = "riscv64")]
    unsafe {
        core::arch::asm!("sfence.vma");
    }
}

///设置页表，创建一个新的satp的值，然后将其写入satp寄存器
///
/// Assign addr to satp.
#[inline]
#[no_mangle]
pub fn set_vspace_root(addr: PAddr, asid: usize) {
    let satp = satp_t::new(8usize, asid, addr.raw() >> 12);
    satp::write(satp.words);
    #[cfg(not(feature = "enable_smp"))]
    sfence();
    #[cfg(feature = "enable_smp")]
    sfence_local();
}
