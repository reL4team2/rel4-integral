pub mod handler;

#[cfg(target_arch = "riscv64")]
use crate::BIT;
#[cfg(target_arch = "riscv64")]
use core::arch::asm;
use sel4_common::platform::*;
use sel4_common::sel4_config::*;
#[cfg(target_arch = "aarch64")]
use sel4_common::utils::global_ops;
use sel4_common::utils::{convert_to_mut_type_ref, cpu_id};
use sel4_common::structures::irq_t;
use sel4_cspace::interface::cte_t;
use sel4_vspace::pptr_t;

#[cfg(target_arch = "riscv64")]
use crate::arch::read_sip;

#[cfg(feature = "ENABLE_SMP")]
use crate::ffi::{ipi_clear_irq, ipi_get_irq};

#[cfg(not(feature = "ENABLE_SMP"))]
pub const MAX_IRQ: usize = maxIRQ;

#[cfg(feature = "ENABLE_SMP")]
pub const MAX_IRQ: usize = (CONFIG_MAX_NUM_NODES - 1) * NUM_PPI + maxIRQ;

#[no_mangle]
pub static mut intStateIRQTable: [usize; MAX_IRQ + 1] = [0; MAX_IRQ + 1];

pub static mut intStateIRQNode_ptr: pptr_t = 0;

#[no_mangle]
// #[link_section = ".boot.bss"]
pub static mut active_irq: [usize; CONFIG_MAX_NUM_NODES] = [0; CONFIG_MAX_NUM_NODES];

#[cfg(feature = "ENABLE_SMP")]
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum IRQState {
    IRQInactive = 0,
    IRQSignal = 1,
    IRQTimer = 2,
    IRQIPI = 3,
    IRQReserved = 4,
}

#[cfg(not(feature = "ENABLE_SMP"))]
#[allow(dead_code)]
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum IRQState {
    IRQInactive = 0,
    IRQSignal = 1,
    IRQTimer = 2,
    IRQReserved = 3,
}

/// 这部分弄得我头都晕了，目前大概情况是这样的
/// intStateIRQTable 存储的是全局的 irq index
/// 当 arm 多核的时候，由于 arm 分为 local 中断号和 global 中断号
/// 导致 irq 和 index 是不一样的，有一个映射关系，通过 idx_to_irq 和 irq_to_idx 转换
/// 那么有的 irq 函数是用 index，有的用 irq，需要进一步区分

/// irq 是从 getActiveIRQ 获取的，统一为输入 irq
#[inline]
pub fn get_irq_state(irq: usize) -> IRQState {
    unsafe { core::mem::transmute::<u8, IRQState>(intStateIRQTable[irq_to_idx(irq)] as u8) }
}

/// 和下面的 delete 都是 index，从 cspace 中删除 slot
#[inline]
pub fn get_irq_handler_slot(irq: usize) -> &'static mut cte_t {
    unsafe { convert_to_mut_type_ref::<cte_t>(intStateIRQNode_ptr).get_offset_slot(irq) }
}

pub fn deletingIRQHandler(irq: usize) {
    get_irq_handler_slot(irq).delete_one()
}

#[no_mangle]
pub fn setIRQState(irq: usize) -> bool {
    panic!("should not be invoked");
}

/// 有的是 index，有的是 irq，在 cspace 和 decode_irq_control_invocation 中是 index，考虑增加一个新函数
pub fn setIRQStateByIrq(state: IRQState, irq: usize) {
    unsafe {
        intStateIRQTable[irq_to_idx(irq)] = state as usize;
    }
    // TODO
    // #if defined ENABLE_SMP_SUPPORT && defined CONFIG_ARCH_ARM
    //     if (IRQ_IS_PPI(irq) && IRQT_TO_CORE(irq) != getCurrentCPUIndex()) {
    //         doRemoteMaskPrivateInterrupt(IRQT_TO_CORE(irq), irqState == IRQInactive, IRQT_TO_IDX(irq));
    //         return;
    //     }
    // #endif
    mask_interrupt(state == IRQState::IRQInactive, irq);
}

pub fn setIRQStateByIndex(state: IRQState, index: usize) {
    unsafe {
        intStateIRQTable[index] = state as usize;
    }

    mask_interrupt(state == IRQState::IRQInactive, idx_to_irq(index));
}

#[repr(align(8192))]
pub struct intStateIRQ_Node([u8; core::mem::size_of::<cte_t>() * 4]);

impl intStateIRQ_Node {
    const fn new() -> Self {
        let buf = [0; core::mem::size_of::<cte_t>() * 4];
        Self(buf)
    }
}
#[no_mangle]
pub(crate) static intStateIRQNode: intStateIRQ_Node = intStateIRQ_Node::new();
#[no_mangle]
pub extern "C" fn intStateIRQNodeToR() {
    unsafe {
        intStateIRQNode_ptr = intStateIRQNode.0.as_ptr() as usize;
    }
}

/// 暂时没用，用的话应该和 deletingIRQHandler 一样，都是 index
#[no_mangle]
pub fn deletedIRQHandler(index: usize) {
    setIRQStateByIndex(IRQState::IRQInactive, index);
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
pub fn isIRQPending() -> bool {
    let sip = read_sip();
    if (sip & (BIT!(SIP_STIP) | BIT!(SIP_SEIP))) != 0 {
        true
    } else {
        false
    }
}

#[cfg(target_arch = "aarch64")]
pub fn isIRQPending() -> bool {
    false
}

/// 毫无疑问，应该是 irq
#[cfg(target_arch = "riscv64")]
#[no_mangle]
pub fn ackInterrupt(irq: usize) {
    unsafe {
        active_irq[cpu_id()] = irqInvalid;
    }
    #[cfg(feature = "ENABLE_SMP")]
    {
        if irq == INTERRUPT_IPI_0 || irq == INTERRUPT_IPI_1 {
            unsafe {
                ipi_clear_irq(irq);
            }
        }
    }
    return;
}

#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub fn ackInterrupt(irq: usize) {
    if crate::arch::arm_gic::gic_v2::irq_is_edge_triggered(irq) {
        crate::arch::arm_gic::gic_v2::dist_pending_clr(irq);
    }
    crate::arch::arm_gic::gic_v2::gic_v2::ack_irq(irq);
    global_ops!(active_irq[cpu_id()] = 0);
    return;
}

/// 同样的问题，decode_irq_control_invocation 中有用到，应该是 index
#[inline]
pub fn is_irq_active(index: usize) -> bool {
    let state = unsafe { core::mem::transmute::<u8, IRQState>(intStateIRQTable[index] as u8) };
    state != IRQState::IRQInactive
}

#[no_mangle]
pub fn isIRQActive(irq: usize) -> bool {
    panic!("should not be invoked");
}

/// 看起来 getActiveIRQ 都是获取当前的 irq
#[cfg(target_arch = "riscv64")]
#[inline]
#[no_mangle]
pub fn getActiveIRQ() -> usize {
    let mut irq = unsafe { active_irq[cpu_id()] };
    if IS_IRQ_VALID(irq) {
        return irq;
    }
    let sip = read_sip();
    #[cfg(feature = "ENABLE_SMP")]
    {
        use sel4_common::arch::riscv64::clear_ipi;
        if (sip & BIT!(SIP_SEIP)) != 0 {
            irq = 0;
        } else if (sip & BIT!(SIP_SSIP)) != 0 {
            clear_ipi();
            irq = unsafe { ipi_get_irq() };
            // debug!("irq: {}", irq);
        } else if (sip & BIT!(SIP_STIP)) != 0 {
            irq = KERNEL_TIMER_IRQ;
        } else {
            irq = irqInvalid;
        }
    }
    #[cfg(not(feature = "ENABLE_SMP"))]
    if (sip & BIT!(SIP_SEIP)) != 0 {
        irq = 0;
    } else if (sip & BIT!(SIP_STIP)) != 0 {
        irq = KERNEL_TIMER_IRQ;
    } else {
        irq = irqInvalid;
    }
    unsafe {
        active_irq[cpu_id()] = irq;
    }
    return irq;
}

#[cfg(target_arch = "aarch64")]
#[inline]
#[no_mangle]
pub fn getActiveIRQ() -> usize {
    /*
        irq_t irq;
        if (!IS_IRQ_VALID(active_irq[CURRENT_CPU_INDEX()])) {
            active_irq[CURRENT_CPU_INDEX()] = gic_cpuiface->int_ack;
        }

        if (IS_IRQ_VALID(active_irq[CURRENT_CPU_INDEX()])) {
            irq = CORE_IRQ_TO_IRQT(CURRENT_CPU_INDEX(), active_irq[CURRENT_CPU_INDEX()] & IRQ_MASK);
        } else {
            irq = irqInvalid;
        }
    */
    use crate::arch::arm_gic::gic_v2::{consts::IRQ_MASK, gic_v2::gic_int_ack};

    if !IS_IRQ_VALID(global_ops!(active_irq[cpu_id()])) {
        global_ops!(active_irq[cpu_id()] = gic_int_ack());
    }
    let irq = match global_ops!(IS_IRQ_VALID(active_irq[cpu_id()])) {
        true => global_ops!(active_irq[cpu_id()] & IRQ_MASK as usize),
        false => irqInvalid,
    };
    log::debug!("active irq: {}", irq);
    irq
}

/// x 是 irq
pub const fn IS_IRQ_VALID(x: usize) -> bool {
    (x <= maxIRQ) && (x != irqInvalid)
}

#[inline]
fn irq_to_idx(irq: usize) -> usize {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "ENABLE_SMP", target_arch = "aarch64"))] {
            use crate::arch::arm_gic::irq_to_idx;
            irq_to_idx(irq_t { core: cpu_id(), irq: irq })
        } else {
            irq as usize
        }
    }
}

#[inline]
fn idx_to_irq(idx: usize) -> usize {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "ENABLE_SMP", target_arch = "aarch64"))] {
            use crate::arch::arm_gic::idx_to_irq;
            idx_to_irq(idx)
        } else {
            idx
        }
    }
}