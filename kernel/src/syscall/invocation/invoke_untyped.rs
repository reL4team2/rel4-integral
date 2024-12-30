// use crate::ffi::tcbDebugAppend;
use super::arch::arch_create_object;
use crate::syscall::{
    FREE_INDEX_TO_OFFSET, GET_FREE_INDEX, GET_OFFSET_FREE_PTR, OFFSET_TO_FREE_IDNEX,
};
use sel4_common::arch::ObjectType;
use sel4_common::structures_gen::{
    cap, cap_cnode_cap, cap_endpoint_cap, cap_notification_cap, cap_thread_cap, cap_untyped_cap,
};
#[cfg(feature = "KERNEL_MCS")]
use sel4_common::structures_gen::{cap_reply_cap, cap_sched_context_cap};
use sel4_common::{
    sel4_config::*, structures::exception_t, utils::convert_to_mut_type_ref, BIT, ROUND_DOWN,
};
use sel4_cspace::deps::preemptionPoint;
use sel4_cspace::interface::{cte_t, insert_new_cap};
use sel4_task::{get_current_domain, tcb_t};
use sel4_vspace::pptr_t;

use crate::utils::*;

fn create_new_objects(
    obj_type: ObjectType,
    parent: &mut cte_t,
    dest_cnode: &mut cte_t,
    dest_offset: usize,
    dest_length: usize,
    region_base: usize,
    user_size: usize,
    device_mem: usize,
) {
    // debug!("create_new_object: {:?}", obj_type);
    let object_size = obj_type.get_object_size(user_size);
    for i in 0..dest_length {
        let capability = create_object(
            obj_type,
            region_base + (i << object_size),
            user_size,
            device_mem,
        );
        insert_new_cap(
            parent,
            dest_cnode.get_offset_slot(dest_offset + i),
            &capability,
        );
    }
}

// #[cfg(target_arch = "riscv64")]
// fn create_object(
//     obj_type: ObjectType,
//     region_base: pptr_t,
//     user_size: usize,
//     device_mem: usize,
// ) -> cap_t {
//     match obj_type {
//         ObjectType::TCBObject => {
//             let tcb = convert_to_mut_type_ref::<tcb_t>(region_base + TCB_OFFSET);
//             tcb.init();
//             tcb.tcbTimeSlice = CONFIG_TIME_SLICE;
//             tcb.domain = get_current_domain();
//             // #[cfg(feature="DEBUG_BUILD")]
//             // unsafe {
//             //     tcbDebugAppend(tcb as *mut tcb_t);
//             // }
//             return cap_t::new_thread_cap(tcb.get_ptr());
//         }

//         ObjectType::EndpointObject => cap_t::new_endpoint_cap(0, 1, 1, 1, 1, region_base),

//         ObjectType::NotificationObject => cap_t::new_notification_cap(0, 1, 1, region_base),

//         ObjectType::CapTableObject => cap_t::new_cnode_cap(user_size, 0, 0, region_base),

//         ObjectType::UnytpedObject => cap_t::new_untyped_cap(0, device_mem, user_size, region_base),
//     }
// }
fn create_object(
    obj_type: ObjectType,
    region_base: pptr_t,
    user_size: usize,
    device_mem: usize,
) -> cap {
    match obj_type {
        ObjectType::TCBObject => {
            let tcb = convert_to_mut_type_ref::<tcb_t>(region_base + TCB_OFFSET);
            tcb.init();
            #[cfg(feature = "KERNEL_MCS")]
            {
                tcb.tcbTimeSlice = CONFIG_TIME_SLICE;
            }
            tcb.domain = get_current_domain();
            // #[cfg(feature="DEBUG_BUILD")]
            // unsafe {
            //     tcbDebugAppend(tcb as *mut tcb_t);
            // }
            return cap_thread_cap::new(tcb.get_ptr() as u64).unsplay();
        }
        ObjectType::CapTableObject => {
            cap_cnode_cap::new(0, 0, user_size as u64, region_base as u64).unsplay()
        }
        ObjectType::NotificationObject => {
            cap_notification_cap::new(0, 1, 1, region_base as u64).unsplay()
        }
        ObjectType::EndpointObject => {
            cap_endpoint_cap::new(0, 1, 1, 1, 1, region_base as u64).unsplay()
        }
        ObjectType::UnytpedObject => {
            cap_untyped_cap::new(0, device_mem as u64, user_size as u64, region_base as u64)
                .unsplay()
        }
        #[cfg(feature = "KERNEL_MCS")]
        ObjectType::SchedContextObject => {
            cap_sched_context_cap::new(region_base as u64, user_size as u64).unsplay()
        }
        #[cfg(feature = "KERNEL_MCS")]
        ObjectType::ReplyObject => cap_reply_cap::new(region_base as u64, 1 as u64).unsplay(),

        _ => arch_create_object(obj_type, region_base, user_size, device_mem),
    }
}

pub fn reset_untyped_cap(srcSlot: &mut cte_t) -> exception_t {
    let prev_cap = &mut cap::cap_untyped_cap(&(*srcSlot).capability);
    let block_size = prev_cap.get_capBlockSize() as usize;
    let region_base = prev_cap.get_capPtr() as usize;
    let chunk = CONFIG_RESET_CHUNK_BITS;
    let offset = FREE_INDEX_TO_OFFSET(prev_cap.get_capFreeIndex() as usize);
    let device_mem = prev_cap.get_capIsDevice();
    if offset == 0 {
        return exception_t::EXCEPTION_NONE;
    }

    if device_mem != 0 && block_size < chunk {
        if device_mem != 0 {
            clear_memory(region_base as *mut u8, block_size);
        }
        prev_cap.set_capFreeIndex(0);
    } else {
        let mut offset: isize = ROUND_DOWN!(offset - 1, chunk) as isize;
        while offset != -(BIT!(chunk) as isize) {
            clear_memory(
                GET_OFFSET_FREE_PTR(region_base, offset as usize) as *mut u8,
                chunk,
            );
            prev_cap.set_capFreeIndex(OFFSET_TO_FREE_IDNEX(offset as usize) as u64);
            let status = unsafe { preemptionPoint() };
            if status != exception_t::EXCEPTION_NONE {
                return status;
            }
            offset -= BIT!(chunk) as isize;
        }
    }
    exception_t::EXCEPTION_NONE
}

pub fn invoke_untyped_retype(
    src_slot: &mut cte_t,
    reset: bool,
    retype_base: pptr_t,
    new_type: ObjectType,
    user_size: usize,
    dest_cnode: &mut cte_t,
    dest_offset: usize,
    dest_length: usize,
    device_mem: usize,
) -> exception_t {
    let region_base = cap::cap_untyped_cap(&src_slot.capability).get_capPtr() as usize;
    if reset {
        let status = reset_untyped_cap(src_slot);
        if status != exception_t::EXCEPTION_NONE {
            return status;
        }
    }
    let total_object_size = dest_length << new_type.get_object_size(user_size);
    let free_ref = retype_base + total_object_size;
    cap::cap_untyped_cap(&src_slot.capability)
        .set_capFreeIndex(GET_FREE_INDEX(region_base, free_ref) as u64);
    create_new_objects(
        new_type,
        src_slot,
        dest_cnode,
        dest_offset,
        dest_length,
        retype_base,
        user_size,
        device_mem,
    );
    exception_t::EXCEPTION_NONE
}
