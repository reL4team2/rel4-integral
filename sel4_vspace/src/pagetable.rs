use crate::PTE;
use sel4_common::structures::paddr_t;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PageTable(pub(crate) paddr_t);

#[allow(unused)]
impl PageTable {
    #[inline]
    pub(crate) fn set(&mut self, value: usize) {
        self.0 = paddr_t::from(value);
    }
    #[inline]
    pub(crate) fn get_pte_list(&mut self) -> &'static mut [PTE] {
        self.0.slice_mut_with_len::<PTE>(Self::PTE_NUM_IN_PAGE)
    }
    #[inline]
    pub(crate) fn base(&self) -> usize {
        self.0.addr()
    }
    #[inline]
    pub(crate) const fn new(paddr: paddr_t) -> Self {
        Self(paddr)
    }

    #[inline]
    pub(crate) fn map_next_table(&mut self, idx: usize, addr: usize, is_leaf: bool) {
        let ptes = self.get_pte_list();
        ptes[idx] = PTE::pte_next_table(addr, is_leaf);
    }
}
