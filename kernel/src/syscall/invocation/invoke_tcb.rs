use sel4_common::arch::*;
use sel4_common::message_info::seL4_MessageInfo_func;
use sel4_common::shared_types_bf_gen::seL4_MessageInfo;
use sel4_common::structures_gen::{cap, cap_thread_cap, notification};
use sel4_common::{
    sel4_config::{TCB_BUFFER, TCB_CTABLE, TCB_VTABLE},
    structures::{exception_t, seL4_IPCBuffer},
};
use sel4_cspace::interface::{cte_insert, cte_t, same_object_as};
use sel4_ipc::Transfer;
use sel4_task::{get_currenct_thread, reschedule_required, set_thread_state, tcb_t, ThreadState};

use crate::syscall::{do_bind_notification, safe_unbind_notification, utils::get_syscall_arg};

pub fn invoke_tcb_read_registers(
    src: &mut tcb_t,
    suspend_source: usize,
    n: usize,
    _arch: usize,
    call: bool,
) -> exception_t {
    let thread = get_currenct_thread();
    if suspend_source != 0 {
        // cancel_ipc(src);
        src.cancel_ipc();
        src.suspend();
    }
    if call {
        let mut op_ipc_buffer = thread.lookup_mut_ipc_buffer(true);
        thread.tcbArch.set_register(ArchReg::Badge, 0);
        let mut i: usize = 0;
        while i < n && i < FRAME_REG_NUM && i < MSG_REGISTER_NUM {
            // setRegister(thread, MSG_REGISTER[i], getRegister(src, FRAME_REGISTERS[i]));
            thread
                .tcbArch
                .set_register(ArchReg::Msg(i), src.tcbArch.get_register(ArchReg::Frame(i)));
            i += 1;
        }

        if let Some(ipc_buffer) = op_ipc_buffer.as_deref_mut() {
            while i < n && i < FRAME_REG_NUM {
                ipc_buffer.msg[i] = src.tcbArch.get_register(ArchReg::Frame(i));
                i += 1;
            }
        }
        let j = i;
        i = 0;
        while i < GP_REG_NUM && i + FRAME_REG_NUM < n && i + FRAME_REG_NUM < MSG_REGISTER_NUM {
            thread.tcbArch.set_register(
                // MSG_REGISTER[i + FRAME_REG_NUM],
                ArchReg::Msg(i + FRAME_REG_NUM),
                src.tcbArch.get_register(ArchReg::GP(i)),
            );
            i += 1;
        }

        if let Some(ipc_buffer) = op_ipc_buffer {
            while i < GP_REG_NUM && i + FRAME_REG_NUM < n {
                ipc_buffer.msg[i + FRAME_REG_NUM] = src.tcbArch.get_register(ArchReg::GP(i));
                i += 1;
            }
        }
        thread.tcbArch.set_register(
            ArchReg::MsgInfo,
            seL4_MessageInfo::new(0, 0, 0, (i + j) as u64).to_word(),
        );
    }
    set_thread_state(thread, ThreadState::ThreadStateRunning);
    exception_t::EXCEPTION_NONE
}

pub fn invoke_tcb_write_registers(
    dest: &mut tcb_t,
    resumeTarget: usize,
    mut n: usize,
    _arch: usize,
    buffer: &seL4_IPCBuffer,
) -> exception_t {
    if n > FRAME_REG_NUM + GP_REG_NUM {
        n = FRAME_REG_NUM + GP_REG_NUM;
    }

    let mut i = 0;
    while i < FRAME_REG_NUM && i < n {
        dest.tcbArch
            .set_register(ArchReg::Frame(i), get_syscall_arg(i + 2, buffer));
        i += 1;
    }
    i = 0;
    while i < GP_REG_NUM && i + FRAME_REG_NUM < n {
        dest.tcbArch.set_register(
            ArchReg::GP(i),
            get_syscall_arg(i + FRAME_REG_NUM + 2, buffer),
        );
        i += 1;
    }

    dest.tcbArch
        .set_register(ArchReg::NextIP, dest.tcbArch.get_register(ArchReg::FaultIP));

    if resumeTarget != 0 {
        // cancel_ipc(dest);
        if dest.is_stopped() {
            dest.cancel_ipc();
        }
        dest.restart();
    }
    if dest.is_current() {
        reschedule_required();
    }
    exception_t::EXCEPTION_NONE
}

pub fn invoke_tcb_copy_registers(
    dest: &mut tcb_t,
    src: &mut tcb_t,
    suspendSource: usize,
    resumeTarget: usize,
    transferFrame: usize,
    _transferInteger: usize,
    _transferArch: usize,
) -> exception_t {
    if suspendSource != 0 {
        // cancel_ipc(src);
        src.cancel_ipc();
        src.suspend();
    }
    if resumeTarget != 0 {
        // cancel_ipc(dest);
        dest.cancel_ipc();
        dest.restart();
    }
    if transferFrame != 0 {
        for i in 0..GP_REG_NUM {
            dest.tcbArch
                .set_register(ArchReg::GP(i), src.tcbArch.get_register(ArchReg::GP(i)));
        }
    }
    if dest.is_current() {
        reschedule_required();
    }
    exception_t::EXCEPTION_NONE
}

#[inline]
pub fn invoke_tcb_suspend(thread: &mut tcb_t) -> exception_t {
    // cancel_ipc(thread);
    thread.cancel_ipc();
    thread.suspend();
    exception_t::EXCEPTION_NONE
}

#[inline]
pub fn invoke_tcb_resume(thread: &mut tcb_t) -> exception_t {
    // cancel_ipc(thread);
    if thread.is_stopped() {
        thread.cancel_ipc();
    }
    thread.restart();
    exception_t::EXCEPTION_NONE
}

#[inline]
pub fn invoke_tcb_set_mcp(target: &mut tcb_t, mcp: usize) -> exception_t {
    target.set_mc_priority(mcp);
    exception_t::EXCEPTION_NONE
}

#[inline]
pub fn invoke_tcb_set_priority(target: &mut tcb_t, prio: usize) -> exception_t {
    target.set_priority(prio);
    exception_t::EXCEPTION_NONE
}
#[cfg(not(feature = "kernel_mcs"))]
pub fn invoke_tcb_set_space(
    target: &mut tcb_t,
    slot: &mut cte_t,
    fault_ep: usize,
    croot_new_cap: &cap,
    croot_src_slot: &mut cte_t,
    vroot_new_cap: &cap,
    vroot_src_slot: &mut cte_t,
) -> exception_t {
    let target_cap = cap_thread_cap::new(target.get_ptr().raw() as u64).unsplay();
    target.TCB_FAULT_HANDLER = fault_ep;
    let root_slot = target.get_cspace_mut_ref(TCB_CTABLE);
    let status = root_slot.delete_all(true);
    if status != exception_t::EXCEPTION_NONE {
        return status;
    }
    if same_object_as(croot_new_cap, &croot_src_slot.capability)
        && same_object_as(&target_cap, &slot.capability)
    {
        cte_insert(croot_new_cap, croot_src_slot, root_slot);
    }

    let root_vslot = target.get_cspace_mut_ref(TCB_VTABLE);
    let status = root_vslot.delete_all(true);
    if status != exception_t::EXCEPTION_NONE {
        return status;
    }
    if same_object_as(vroot_new_cap, &vroot_src_slot.capability)
        && same_object_as(&target_cap, &slot.capability)
    {
        cte_insert(vroot_new_cap, vroot_src_slot, root_vslot);
    }
    exception_t::EXCEPTION_NONE
}
#[cfg(feature = "kernel_mcs")]
#[no_mangle]
pub fn install_tcb_cap(
    target: &mut tcb_t,
    tCap: &cap,
    slot: &mut cte_t,
    index: usize,
    newCap: &cap,
    srcSlot: &mut cte_t,
) -> exception_t {
    let mut rootSlot = target.get_cspace_mut_ref(index);
    let e = rootSlot.delete_all(true);
    if e != exception_t::EXCEPTION_NONE {
        return e;
    }
    if same_object_as(newCap, &srcSlot.capability) && same_object_as(tCap, &slot.capability) {
        cte_insert(newCap, srcSlot, &mut rootSlot);
    }
    return e;
}
#[cfg(feature = "kernel_mcs")]
pub fn invoke_tcb_thread_control_caps(
    target: &mut tcb_t,
    slot: &mut cte_t,
    fh_newCap: &cap,
    fh_srcSlot: Option<&mut cte_t>,
    th_newCap: &cap,
    th_srcSlot: Option<&mut cte_t>,
    croot_new_cap: &cap,
    croot_src_slot: Option<&mut cte_t>,
    vroot_new_cap: &cap,
    vroot_src_slot: Option<&mut cte_t>,
    updateFlags: usize,
) -> exception_t {
    use sel4_common::sel4_config::{
        TCB_FAULT_HANDLER, TCB_TIMEOUT_HANDLER, THREAD_CONTROL_CAPS_UPDATE_FAULT,
        THREAD_CONTROL_CAPS_UPDATE_SPACE, THREAD_CONTROL_CAPS_UPDATE_TIMEOUT,
    };
    let target_cap = cap_thread_cap::new(target.get_ptr().as_u64()).unsplay();
    if updateFlags & THREAD_CONTROL_CAPS_UPDATE_FAULT != 0 {
        if let Some(fh_slot) = fh_srcSlot {
            let e = install_tcb_cap(
                target,
                &target_cap,
                slot,
                TCB_FAULT_HANDLER,
                fh_newCap,
                fh_slot,
            );
            if e != exception_t::EXCEPTION_NONE {
                return e;
            }
        }
    }
    if updateFlags & THREAD_CONTROL_CAPS_UPDATE_TIMEOUT != 0 {
        if let Some(th_slot) = th_srcSlot {
            let e = install_tcb_cap(
                target,
                &target_cap,
                slot,
                TCB_TIMEOUT_HANDLER,
                th_newCap,
                th_slot,
            );
            if e != exception_t::EXCEPTION_NONE {
                return e;
            }
        }
    }
    if updateFlags & THREAD_CONTROL_CAPS_UPDATE_SPACE != 0 {
        if let Some(croot_slot) = croot_src_slot {
            let e = install_tcb_cap(
                target,
                &target_cap,
                slot,
                TCB_CTABLE,
                croot_new_cap,
                croot_slot,
            );
            if e != exception_t::EXCEPTION_NONE {
                return e;
            }
        }
        if let Some(vroot_slot) = vroot_src_slot {
            let e = install_tcb_cap(
                target,
                &target_cap,
                slot,
                TCB_VTABLE,
                vroot_new_cap,
                vroot_slot,
            );
            if e != exception_t::EXCEPTION_NONE {
                return e;
            }
        }
    }

    // target.TCB_FAULT_HANDLER = fault_ep;
    // let root_slot = target.get_cspace_mut_ref(TCB_CTABLE);
    // let status = root_slot.delete_all(true);
    // if status != exception_t::EXCEPTION_NONE {
    //     return status;
    // }
    // if same_object_as(croot_new_cap, &croot_src_slot.capability)
    //     && same_object_as(&target_cap, &slot.capability)
    // {
    //     cte_insert(croot_new_cap, croot_src_slot, root_slot);
    // }

    // let root_vslot = target.get_cspace_mut_ref(TCB_VTABLE);
    // let status = root_vslot.delete_all(true);
    // if status != exception_t::EXCEPTION_NONE {
    //     return status;
    // }
    // if same_object_as(vroot_new_cap, &vroot_src_slot.capability)
    //     && same_object_as(&target_cap, &slot.capability)
    // {
    //     cte_insert(vroot_new_cap, vroot_src_slot, root_vslot);
    // }
    exception_t::EXCEPTION_NONE
}

pub fn invoke_tcb_set_ipc_buffer(
    target: &mut tcb_t,
    slot: &mut cte_t,
    buffer_addr: usize,
    buffer_cap: cap,
    buffer_src_slot: Option<&mut cte_t>,
) -> exception_t {
    let target_cap = cap_thread_cap::new(target.get_ptr().raw() as u64).unsplay();
    let buffer_slot = target.get_cspace_mut_ref(TCB_BUFFER);
    let status = buffer_slot.delete_all(true);
    if status != exception_t::EXCEPTION_NONE {
        return status;
    }
    target.tcbIPCBuffer = vptr!(buffer_addr);
    if let Some(mut buffer_src_slot) = buffer_src_slot {
        if same_object_as(&buffer_cap, &buffer_src_slot.capability)
            && same_object_as(&target_cap, &slot.capability)
        {
            cte_insert(&buffer_cap, &mut buffer_src_slot, buffer_slot);
        }
    }
    if target.is_current() {
        reschedule_required();
    }
    exception_t::EXCEPTION_NONE
}

#[inline]
pub fn invoke_tcb_bind_notification(tcb: &mut tcb_t, ntfn: &mut notification) -> exception_t {
    do_bind_notification(tcb, ntfn);
    exception_t::EXCEPTION_NONE
}

#[inline]
pub fn invoke_tcb_unbind_notification(tcb: &mut tcb_t) -> exception_t {
    safe_unbind_notification(tcb);
    exception_t::EXCEPTION_NONE
}

#[inline]
pub fn invoke_tcb_set_tls_base(thread: &mut tcb_t, base: usize) -> exception_t {
    thread.tcbArch.set_register(ArchReg::TlsBase, base);
    if thread.is_current() {
        reschedule_required();
    }
    exception_t::EXCEPTION_NONE
}

#[cfg(all(feature = "enable_smp", not(feature = "kernel_mcs")))]
#[inline]
pub fn invoke_tcb_set_affinity(thread: &mut tcb_t, affinitiy: usize) -> exception_t {
    thread.sched_dequeue();
    crate::smp::migrate_tcb(thread, affinitiy);
    thread.tcbAffinity = affinitiy;
    // debug!("tcb migrate: {}", thread.tcbAffinity);
    if thread.is_runnable() {
        thread.sched_append();
    }

    if thread.is_current() {
        reschedule_required();
    }
    exception_t::EXCEPTION_NONE
}
