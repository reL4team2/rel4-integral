use crate::syscall::invocation::decode::arch::decode_mmu_invocation;
use sel4_common::arch::MessageLabel;
use sel4_common::sel4_config::TCB_VTABLE;
use sel4_common::structures::exception_t;
use sel4_common::structures::seL4_IPCBuffer;
use sel4_common::structures_gen::cap;
use sel4_cspace::capability::cap_arch_func;
use sel4_cspace::interface::cte_t;
use sel4_task::get_currenct_thread;
use sel4_vspace::asid_t;
use sel4_vspace::set_current_user_vspace_root;
use sel4_vspace::ttbr_new;

#[no_mangle]
// typedef word_t cptr_t;
extern "C" fn decodeARMMMUInvocation(
    invLabel: MessageLabel,
    length: usize,
    _cptr: usize,
    cte: &mut cte_t,
    _cap: cap,
    call: bool,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    decode_mmu_invocation(invLabel, length, cte, call, buffer)
}

/// Set VMRoot and flush if necessary
pub fn set_vm_root_for_flush(vspace: usize, asid: asid_t) -> bool {
    let thread_root = &get_currenct_thread().get_cspace(TCB_VTABLE).capability;

    if thread_root.is_valid_native_root()
        && cap::cap_vspace_cap(&thread_root).get_capVSBasePtr() == vspace as u64
    {
        return false;
    }

    set_current_user_vspace_root(ttbr_new(asid, paddr!(vspace)));
    true
}
