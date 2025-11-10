use core::ops::Range;

use crate::aarch64::tcb::ArchTCB;

/// ArchTCB Common part
impl ArchTCB {
    // /// Set the register of the TCB
    // /// # Arguments
    // /// * `reg` - The register index.
    // /// * `w` - The value to set.
    // #[inline]
    // pub fn set_register(&mut self, reg: ArchReg, w: usize) {
    //     self.registers[reg.to_index()] = w;
    // }

    // /// Get the register value of the TCB
    // /// # Arguments
    // /// * `reg` - The register index.
    // /// # Returns
    // /// The value of the register.
    // #[inline]
    // pub const fn get_register(&self, reg: ArchReg) -> usize {
    //     self.registers[reg.to_index()]
    // }

    /// Copy the value of a range from source TCB to destination TCB
    #[inline]
    pub fn copy_range(&mut self, _source: &Self, _range: Range<usize>) {
        todo!("")
        // self.registers[range.clone()].copy_from_slice(&source.registers[range]);
    }

    /// Get the raw pointer of the TCB
    ///
    /// Used in the `restore_user_context`
    #[inline]
    pub fn raw_ptr(&self) -> usize {
        self as *const ArchTCB as usize
    }
}
