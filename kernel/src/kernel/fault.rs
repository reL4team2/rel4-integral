use sel4_common::{arch::n_syscallMessage, structures_gen::seL4_Fault};
use sel4_task::*;

#[no_mangle]
pub fn process3(sender: *mut tcb_t, receiver: *mut tcb_t, _receiveIPCBuffer: *mut usize) -> usize {
    unsafe {
        (*sender).copy_syscall_fault_mrs(&mut *receiver);
        (*receiver).set_mr(
            n_syscallMessage,
            seL4_Fault::seL4_Fault_UnknownSyscall(&(*sender).tcbFault).get_syscallNumber() as usize,
        )
    }
}
