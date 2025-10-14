use crate::PTE;
use rel4_arch::basic::PAddr;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PageTable(pub(crate) PAddr);

#[allow(unused)]
impl PageTable {
    #[inline]
    pub(crate) fn set(&mut self, value: PAddr) {
        self.0 = value;
    }

    #[inline]
    pub(crate) fn get_pte_list(&mut self) -> &'static mut [PTE] {
        self.0
            .to_pptr()
            .get_mut_slice::<{ Self::PTE_NUM_IN_PAGE }, _>()
    }

    #[inline]
    pub(crate) fn base(&self) -> PAddr {
        self.0
    }

    #[inline]
    pub(crate) const fn new(paddr: PAddr) -> Self {
        Self(paddr)
    }

    #[inline]
    pub(crate) fn map_next_table(&mut self, idx: usize, addr: PAddr, is_leaf: bool) {
        let ptes = self.get_pte_list();
        ptes[idx] = PTE::pte_next_table(addr, is_leaf);
    }
}
