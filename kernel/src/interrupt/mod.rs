pub mod handler;

#[cfg(target_arch = "riscv64")]
use crate::BIT;
#[cfg(target_arch = "riscv64")]
use core::arch::asm;
use sel4_common::platform::*;
use sel4_common::sel4_config::*;
#[cfg(target_arch = "aarch64")]
use sel4_common::utils::{global_ops, unsafe_ops};
use sel4_common::utils::{convert_to_mut_type_ref, cpu_id};
use sel4_common::structures::{current_cpu_irq_to_idx, idx_to_irq};
use sel4_cspace::interface::cte_t;
use sel4_vspace::pptr_t;

#[cfg(target_arch = "riscv64")]
use crate::arch::read_sip;
#[cfg(target_arch = "riscv64")]
use crate::arch::{ipi_get_irq, ipi_clear_irq};

#[cfg(target_arch = "aarch64")]
use crate::arch::arm_gic::gic_v2::{consts::{IRQ_MASK, IRQ_NONE}, gic_v2::gic_int_ack};

cfg_if::cfg_if! {
    if #[cfg(all(feature = "enable_smp", target_arch = "aarch64"))] {
        pub const INT_STATE_ARRAY_SIZE: usize = (CONFIG_MAX_NUM_NODES - 1) * NUM_PPI + MAX_IRQ;
    } else {
        pub const INT_STATE_ARRAY_SIZE: usize = MAX_IRQ;
    }
}

#[no_mangle]
pub static mut int_state_irq_table: [usize; INT_STATE_ARRAY_SIZE + 1] =
    [0; INT_STATE_ARRAY_SIZE + 1];

pub static mut int_state_irq_node_ptr: pptr_t = 0;

#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub static mut active_irq: [usize; CONFIG_MAX_NUM_NODES] = [IRQ_NONE as usize; CONFIG_MAX_NUM_NODES];

#[cfg(target_arch = "riscv64")]
#[no_mangle]
pub static mut active_irq: [usize; CONFIG_MAX_NUM_NODES] = [IRQ_INVALID; CONFIG_MAX_NUM_NODES];

#[cfg(feature = "enable_smp")]
#[allow(dead_code)]
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum IRQState {
    IRQInactive = 0,
    IRQSignal = 1,
    IRQTimer = 2,
    IRQIPI = 3,
    IRQReserved = 4,
}

#[cfg(not(feature = "enable_smp"))]
#[allow(dead_code)]
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum IRQState {
    IRQInactive = 0,
    IRQSignal = 1,
    IRQTimer = 2,
    IRQReserved = 3,
}

/// 这部分弄得我头都晕了，目前大概情况是这样的
/// int_state_irq_table 存储的是全局的 irq index
/// 当 arm 多核的时候，由于 arm 分为 local 中断号和 global 中断号
/// 导致 irq 和 index 是不一样的，有一个映射关系，通过 idx_to_irq 和 irq_to_idx 转换
/// 那么有的 irq 函数是用 index，有的用 irq，需要进一步区分

/// irq 是从 get_active_irq 获取的，统一为输入 irq
#[inline]
pub fn get_irq_state(irq: usize) -> IRQState {
    unsafe { core::mem::transmute::<u8, IRQState>(int_state_irq_table[current_cpu_irq_to_idx(irq)] as u8) }
}

/// 和下面的 delete 都是 index，从 cspace 中删除 slot
#[inline]
pub fn get_irq_handler_slot(irq: usize) -> &'static mut cte_t {
    unsafe { convert_to_mut_type_ref::<cte_t>(int_state_irq_node_ptr).get_offset_slot(irq) }
}

pub fn deleting_irq_handler(irq: usize) {
    get_irq_handler_slot(irq).delete_one()
}

#[no_mangle]
pub fn setIRQState(_irq: usize) -> bool {
    panic!("should not be invoked");
}

/// 有的是 index，有的是 irq，在 cspace 和 decode_irq_control_invocation 中是 index，考虑增加一个新函数
pub fn set_irq_state_by_irq(state: IRQState, irq: usize) {
    unsafe {
        int_state_irq_table[current_cpu_irq_to_idx(irq)] = state as usize;
    }
    mask_interrupt(state == IRQState::IRQInactive, irq);
}

pub fn set_irq_state_by_index(state: IRQState, index: usize) {
    unsafe {
        int_state_irq_table[index] = state as usize;
    }

    #[cfg(all(feature = "enable_smp", target_arch = "aarch64"))]
    {
        use sel4_common::structures::idx_to_irqt;
        use crate::arch::remote_mask_private_interrupt;
        let irq = idx_to_irqt(index);
        if irq.irq < NUM_PPI && irq.core != cpu_id() {
            remote_mask_private_interrupt(irq.core, state == IRQState::IRQInactive, irq.irq);
            return;
        }
    }

    mask_interrupt(state == IRQState::IRQInactive, idx_to_irq(index));
}

#[repr(align(8192))]
pub struct IntStateIrqNode([u8; core::mem::size_of::<cte_t>() * 4]);

impl IntStateIrqNode {
    const fn new() -> Self {
        let buf = [0; core::mem::size_of::<cte_t>() * 4];
        Self(buf)
    }
}
#[no_mangle]
pub(crate) static intStateIRQNode: IntStateIrqNode = IntStateIrqNode::new();
#[no_mangle]
pub extern "C" fn intStateIRQNodeToR() {
    unsafe {
        int_state_irq_node_ptr = intStateIRQNode.0.as_ptr() as usize;
    }
}

/// 暂时没用，用的话应该和 deleting_irq_handler 一样，都是 index
#[no_mangle]
pub fn deletedIRQHandler(index: usize) {
    set_irq_state_by_index(IRQState::IRQInactive, index);
}
#[inline]
#[cfg(target_arch = "riscv64")]
pub fn set_sie_mask(_mask_high: usize) {
    unsafe {
        let _temp: usize;
        asm!("csrrs {0},sie,{1}",out(reg)_temp,in(reg)_mask_high);
    }
}
#[inline]
#[cfg(target_arch = "riscv64")]
pub fn clear_sie_mask(_mask_low: usize) {
    unsafe {
        let _temp: usize;
        asm!("csrrc {0},sie,{1}",out(reg)_temp,in(reg)_mask_low);
    }
}

/// 毫无疑问，应该是 irq
#[inline]
pub fn mask_interrupt(disable: bool, irq: usize) {
    #[cfg(target_arch = "riscv64")]
    if irq == KERNEL_TIMER_IRQ {
        if disable {
            clear_sie_mask(BIT!(SIE_STIE));
        } else {
            set_sie_mask(BIT!(SIE_STIE));
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if disable {
            crate::arch::arm_gic::gic_v2::irq_disable(irq);
        } else {
            crate::arch::arm_gic::gic_v2::irq_enable(irq);
        }
    }
}

#[cfg(target_arch = "riscv64")]
#[inline]
pub fn is_irq_pending() -> bool {
    let sip = read_sip();
    if (sip & (BIT!(SIP_STIP) | BIT!(SIP_SEIP))) != 0 {
        true
    } else {
        false
    }
}

#[cfg(target_arch = "aarch64")]
pub fn is_irq_pending() -> bool {
    false
}

/// 毫无疑问，应该是 irq
#[cfg(target_arch = "riscv64")]
#[no_mangle]
#[cfg_attr(not(feature = "enable_smp"), allow(unused_variables))]
pub fn ack_interrupt(irq: usize) {
    unsafe {
        active_irq[cpu_id()] = IRQ_INVALID;
    }
    #[cfg(feature = "enable_smp")]
    {
        if irq == INTERRUPT_IPI_0 || irq == INTERRUPT_IPI_1 {
            ipi_clear_irq(irq);
        }
    }
    return;
}

#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub fn ack_interrupt(irq: usize) {
    if crate::arch::arm_gic::gic_v2::irq_is_edge_triggered(irq) {
        crate::arch::arm_gic::gic_v2::dist_pending_clr(irq);
    }
    crate::arch::arm_gic::gic_v2::gic_v2::ack_irq(irq);
    global_ops!(active_irq[cpu_id()] = IRQ_NONE as usize);
    return;
}

/// 同样的问题，decode_irq_control_invocation 中有用到，应该是 index
#[inline]
pub fn is_irq_active(index: usize) -> bool {
    let state = unsafe { core::mem::transmute::<u8, IRQState>(int_state_irq_table[index] as u8) };
    state != IRQState::IRQInactive
}

// Do not change it
#[no_mangle]
pub fn isIRQActive(_irq: usize) -> bool {
    panic!("should not be invoked");
}

/// 看起来 get_active_irq 都是获取当前的 irq
#[cfg(target_arch = "riscv64")]
#[inline]
#[no_mangle]
pub fn get_active_irq() -> usize {
    let mut irq = unsafe { active_irq[cpu_id()] };
    if is_irq_valid(irq) {
        return irq;
    }
    let sip = read_sip();
    #[cfg(feature = "enable_smp")]
    {
        use sel4_common::arch::riscv64::clear_ipi;
        if (sip & BIT!(SIP_SEIP)) != 0 {
            irq = 0;
        } else if (sip & BIT!(SIP_SSIP)) != 0 {
            clear_ipi();
            irq = ipi_get_irq();
            // debug!("irq: {}", irq);
        } else if (sip & BIT!(SIP_STIP)) != 0 {
            irq = KERNEL_TIMER_IRQ;
        } else {
            irq = IRQ_INVALID;
        }
    }
    #[cfg(not(feature = "enable_smp"))]
    if (sip & BIT!(SIP_SEIP)) != 0 {
        irq = 0;
    } else if (sip & BIT!(SIP_STIP)) != 0 {
        irq = KERNEL_TIMER_IRQ;
    } else {
        irq = IRQ_INVALID;
    }
    unsafe {
        active_irq[cpu_id()] = irq;
    }
    return irq;
}

#[cfg(target_arch = "aarch64")]
#[inline]
#[no_mangle]
pub fn get_active_irq() -> usize {
    /*
        irq_t irq;
        if (!is_irq_valid(active_irq[CURRENT_CPU_INDEX()])) {
            active_irq[CURRENT_CPU_INDEX()] = gic_cpuiface->int_ack;
        }

        if (is_irq_valid(active_irq[CURRENT_CPU_INDEX()])) {
            irq = CORE_IRQ_TO_IRQT(CURRENT_CPU_INDEX(), active_irq[CURRENT_CPU_INDEX()] & IRQ_MASK);
        } else {
            irq = IRQ_INVALID;
        }
    */
    let irq = gic_int_ack();

    if (irq & IRQ_MASK as usize) < MAX_IRQ {
        unsafe_ops!(active_irq[cpu_id()] = irq);
    }

    let local_irq = unsafe_ops!(active_irq[cpu_id()]) & IRQ_MASK as usize;
    let irq2 = match local_irq < MAX_IRQ {
        true => local_irq,
        false => IRQ_INVALID,
    };
    log::debug!("active irq: {}", irq);
    irq2
}

/// x 是 irq
#[inline]
#[allow(dead_code)]
#[cfg_attr(target_arch = "aarch64", allow(unused_variables))]
pub const fn is_irq_valid(x: usize) -> bool {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "aarch64")] {
            // TODO: not used now
            panic!("not used in aarch64")
        } else {
            (x <= MAX_IRQ) && (x != IRQ_INVALID)
        }
    }
}
