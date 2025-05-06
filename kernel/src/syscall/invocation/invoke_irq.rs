use sel4_common::{
    structures::exception_t,
    structures_gen::{cap, cap_irq_handler_cap},
};
use sel4_cspace::interface::{cte_insert, cte_t};

use crate::interrupt::{get_irq_handler_slot, set_irq_state_by_index, IrqState};

pub fn invoke_irq_control(
    irq: usize,
    handler_slot: &mut cte_t,
    control_slot: &mut cte_t,
) -> exception_t {
    set_irq_state_by_index(IrqState::IRQSignal, irq);
    cte_insert(
        &cap_irq_handler_cap::new(irq as u64).unsplay(),
        control_slot,
        handler_slot,
    );
    exception_t::EXCEPTION_NONE
}

#[inline]
pub fn invoke_set_irq_handler(irq: usize, capability: &cap, slot: &mut cte_t) {
    let irq_slot = get_irq_handler_slot(irq);
    irq_slot.delete_one();
    cte_insert(capability, slot, irq_slot);
}

#[inline]
pub fn invoke_clear_irq_handler(irq: usize) {
    get_irq_handler_slot(irq).delete_one();
}
