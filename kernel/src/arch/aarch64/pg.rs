use crate::syscall::invocation::decode::arch::decode_mmu_invocation;
use sel4_common::arch::MessageLabel;
use sel4_common::sel4_config::tcbVTable;
use sel4_common::structures::exception_t;
use sel4_common::structures::seL4_IPCBuffer;
use sel4_common::structures_gen::cap;
use sel4_cspace::capability::cap_arch_func;
use sel4_cspace::interface::cte_t;
use sel4_task::get_currenct_thread;
use sel4_vspace::asid_t;
use sel4_vspace::setCurrentUserVSpaceRoot;
use sel4_vspace::ttbr_new;
use sel4_vspace::{vptr_t, PTE};

#[repr(C)]
struct lookupPGDSlot_ret_t {
    status: exception_t,
    pgdSlot: usize, // *mut pgde_t
}

#[repr(C)]
struct lookupPDSlot_ret_t {
    status: exception_t,
    pdSlot: usize, // *mut pde_t
}

#[repr(C)]
struct lookupPUDSlot_ret_t {
    status: exception_t,
    pudSlot: usize, // *mut pude_t
}

#[no_mangle]
extern "C" fn lookupPGDSlot(_vspace: *mut PTE, _vptr: vptr_t) -> lookupPGDSlot_ret_t {
    // which is realized under sel4_vspace/src/arch/aarch64/pte.rs as a member function of PTE in this commit
    // ZhiyuanSue
    unimplemented!("lookupPGDSlot")
}

#[no_mangle]
extern "C" fn lookupPDSlot(_vspace: *mut PTE, _vptr: vptr_t) -> lookupPDSlot_ret_t {
    // which is realized under sel4_vspace/src/arch/aarch64/pte.rs as a member function of PTE in this commit
    // ZhiyuanSue
    unimplemented!("lookupPDSlot")
}

#[no_mangle]
extern "C" fn lookupPUDSlot(_vspace: *mut PTE, _vptr: vptr_t) -> lookupPUDSlot_ret_t {
    // which is realized under sel4_vspace/src/arch/aarch64/pte.rs as a member function of PTE in this commit
    // ZhiyuanSue
    unimplemented!("lookupPUDSlot")
}

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
    let thread_root = &get_currenct_thread().get_cspace(tcbVTable).capability;

    if thread_root.is_valid_native_root()
        && cap::cap_vspace_cap(&thread_root).get_capVSBasePtr() == vspace as u64
    {
        return false;
    }

    setCurrentUserVSpaceRoot(ttbr_new(asid, vspace));
    true
}
