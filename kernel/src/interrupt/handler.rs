use sel4_common::platform::{irqInvalid, maxIRQ};
use crate::interrupt::*;
use core::intrinsics::unlikely;
use log::debug;
use sel4_common::platform::{timer, Timer_func};
use sel4_common::structures::exception_t;
use sel4_common::structures_gen::{cap, cap_tag, notification};
use sel4_ipc::notification_func;
use sel4_task::{activateThread, schedule, timerTick};

#[cfg(feature = "KERNEL_MCS")]
use sel4_task::ksReprogram;
#[cfg(feature = "KERNEL_MCS")]
use sel4_task::{checkBudget, updateTimestamp};

#[no_mangle]
pub fn handleInterruptEntry() -> exception_t {
    #[cfg(feature = "KERNEL_MCS")]
    {
        updateTimestamp();
        checkBudget();
    }
    let irq = getActiveIRQ();

    if irq != irqInvalid {
        handleInterrupt(irq);
    }

    schedule();
    activateThread();
    exception_t::EXCEPTION_NONE
}

#[no_mangle]
pub fn handleInterrupt(irq: usize) {
    if unlikely(irq > maxIRQ) {
        debug!(
            "Received IRQ {}, which is above the platforms maxIRQ of {}\n",
            irq, maxIRQ
        );
        mask_interrupt(true, irq);
        ackInterrupt(irq);
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
            #[cfg(feature = "KERNEL_MCS")]
            {
                timer.ackDeadlineIRQ();
                unsafe { ksReprogram = true };
            }
            timerTick();
            timer.resetTimer();
        }
        #[cfg(feature = "ENABLE_SMP")]
        IRQState::IRQIPI => {
            unsafe { crate::ffi::handleIPI(irq, true) };
        }
        IRQState::IRQReserved => {
            debug!("Received unhandled reserved IRQ: {}\n", irq);
        }
    }
    ackInterrupt(irq);
}
