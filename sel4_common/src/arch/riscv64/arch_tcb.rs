use crate::ffi::kernel_stack_alloc;
// use crate::idle_thread;
use super::{fault_messages, msgRegister, NextIP};
use super::{sp, CONTEXT_REG_NUM, SSTATUS, SSTATUS_SPIE, SSTATUS_SPP};
use crate::sel4_config::CONFIG_KERNEL_STACK_BITS;
use crate::BIT;

#[repr(C)]
#[derive(Debug, PartialEq, Clone)]
pub struct FPUState {
    #[cfg(feature = "RISCV_EXT_D")]
    pub regs: [u64; 32],
    #[cfg(feature = "RISCV_EXT_F")]
    pub regs: [u32; 32],
    pub fcsr: u32,
}
/// This is `arch_tcb_t` in the sel4_c_impl.
#[repr(C)]
#[derive(Debug, PartialEq, Clone)]
pub struct ArchTCB {
    pub(in crate::arch) registers: [usize; CONTEXT_REG_NUM],
    pub(in crate::arch) fpu: FPUState,
}

impl Default for ArchTCB {
    fn default() -> Self {
        let mut registers = [0; CONTEXT_REG_NUM];
        registers[SSTATUS] = 0x00040020;
        Self {
            registers,
            fpu: FPUState {
                regs: [0; 32],
                fcsr: 0,
            },
        }
    }
}

impl ArchTCB {
    /// Config the registers fot the idle thread.
    pub fn config_idle_thread(&mut self, idle_thread: usize, core: usize) {
        self.registers[NextIP] = idle_thread;
        self.registers[SSTATUS] = SSTATUS_SPP | SSTATUS_SPIE;
        self.registers[sp] = unsafe {
            &kernel_stack_alloc.data[core][BIT!(CONFIG_KERNEL_STACK_BITS) - 1] as *const u8
        } as usize;
    }

    #[inline]
    pub fn fpu_state_ptr(&mut self) -> *const FPUState {
        &self.fpu as *const FPUState
    }
}
