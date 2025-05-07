use crate::interrupt::*;
use core::intrinsics::unlikely;
use log::debug;
use sel4_common::platform::{timer, Timer_func};
use sel4_common::platform::{IRQ_INVALID, MAX_IRQ};
use sel4_common::structures::exception_t;
use sel4_common::structures_gen::{cap, cap_tag, notification};
use sel4_ipc::notification_func;
use sel4_task::{activateThread, schedule, timer_tick};

#[cfg(feature = "kernel_mcs")]
use sel4_task::ksReprogram;
#[cfg(feature = "kernel_mcs")]
use sel4_task::{check_budget, update_timestamp};

#[no_mangle]
pub fn handle_interrupt_entry() -> exception_t {
    #[cfg(feature = "kernel_mcs")]
    {
        update_timestamp();
        check_budget();
    }
    let irq = get_active_irq();

    if irq != IRQ_INVALID {
        handle_interrput(irq);
    }

    schedule();
    activateThread();
    exception_t::EXCEPTION_NONE
}

#[no_mangle]
pub fn handle_interrput(irq: usize) {
    if unlikely(irq > MAX_IRQ) {
        debug!(
            "Received IRQ {}, which is above the platforms MAX_IRQ of {}\n",
            irq, MAX_IRQ
        );
        mask_interrupt(true, irq);
        ack_interrupt(irq);
        return;
    }
    match get_irq_state(irq) {
        IRQState::IRQInactive => {
            debug!("IRQInactive");
            mask_interrupt(true, irq);
            debug!("Received disabled IRQ: {}\n", irq);
        }
        IRQState::IRQSignal => {
            debug!("IRQSignal");
            let handler_slot = get_irq_handler_slot(irq);
            let handler_cap = &handler_slot.capability;
            if handler_cap.get_tag() == cap_tag::cap_notification_cap
                && cap::cap_notification_cap(handler_cap).get_capNtfnCanSend() != 0
            {
                let nf = convert_to_mut_type_ref::<notification>(
                    cap::cap_notification_cap(handler_cap).get_capNtfnPtr() as usize,
                );
                nf.send_signal(cap::cap_notification_cap(handler_cap).get_capNtfnBadge() as usize);
            }
            #[cfg(not(target_arch = "riscv64"))]
            {
                mask_interrupt(true, irq);
            }
        }
        IRQState::IRQTimer => {
            #[cfg(feature = "kernel_mcs")]
            {
                timer.ack_deadline_irq();
                unsafe { ksReprogram = true };
            }
            timer_tick();
            timer.reset_timer();
        }
        #[cfg(feature = "enable_smp")]
        IRQState::IRQIPI => {
            use sel4_common::structures::irq_t;
            #[cfg(target_arch = "aarch64")]
            unsafe {
                crate::ffi::handleIPI(
                    irq_t {
                        core: sel4_common::utils::cpu_id(),
                        irq,
                    },
                    true,
                )
            };
            #[cfg(target_arch = "riscv64")]
            unsafe {
                crate::ffi::handleIPI(irq as irq_t, true)
            };
        }
        IRQState::IRQReserved => {
            debug!("Received unhandled reserved IRQ: {}\n", irq);
        }
    }
    ack_interrupt(irq);
}
