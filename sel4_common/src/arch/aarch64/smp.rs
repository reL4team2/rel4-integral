use crate::BIT;
use core::arch::asm;

#[inline]
pub fn cpu_index_to_id(index: usize) -> usize {
    return BIT!(index);
}

#[inline]
pub fn get_current_cpu_index() -> usize {
    unsafe {
        let mut id: usize;
        asm!(
            "mrs {},tpidr_el1",
            out(reg) id,
        );
        id & 0xfff
    }
}