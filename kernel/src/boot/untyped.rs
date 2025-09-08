use super::{ndks_boot, utils::*};
use crate::structures::{seL4_SlotPos, SlotRegion, UntypedDesc};

use log::debug;
use rel4_arch::basic::{PRegion, Region};
use sel4_common::sel4_config::*;
use sel4_common::{arch::config::MAX_UNTYPED_BITS, utils::max_free_index};
use sel4_common::{
    sel4_config::SEL4_MIN_UNTYPED_BITS,
    structures_gen::{cap_cnode_cap, cap_untyped_cap},
};

pub fn create_untypeds(root_cnode_cap: &cap_cnode_cap, boot_mem_reuse_reg: Region) -> bool {
    unsafe {
        let first_untyped_slot = ndks_boot.slot_pos_cur;
        let mut start = 0;
        for i in 0..ndks_boot.resv_count {
            let reg = PRegion::new(paddr!(start), ndks_boot.reserved[i].start).to_region();
            if !create_untypeds_for_region(root_cnode_cap, true, reg.clone(), first_untyped_slot) {
                debug!(
                    "ERROR: creation of untypeds for device region {} at
                       [{}..{}] failed\n",
                    i,
                    reg.start.raw(),
                    reg.end.raw()
                );
                return false;
            }
            start = ndks_boot.reserved[i].end.raw();
        }

        if start < CONFIG_PADDR_USER_DEVICE_TOP {
            let reg = PRegion::new(paddr!(start), paddr!(CONFIG_PADDR_USER_DEVICE_TOP)).to_region();
            if !create_untypeds_for_region(root_cnode_cap, true, reg.clone(), first_untyped_slot) {
                debug!(
                    "ERROR: creation of untypeds for top device region 
                       [{}..{}] failed\n",
                    reg.start.raw(),
                    reg.end.raw()
                );
                return false;
            }
        }
        if !create_untypeds_for_region(
            root_cnode_cap,
            false,
            boot_mem_reuse_reg,
            first_untyped_slot,
        ) {
            debug!(
                "ERROR: creation of untypeds for recycled boot memory
                   [{}..{}] failed\n",
                boot_mem_reuse_reg.start.raw(),
                boot_mem_reuse_reg.end.raw()
            );
            return false;
        }

        for i in 0..ndks_boot.freemem.len() {
            let reg = ndks_boot.freemem[i];
            ndks_boot.freemem[i] = Region::empty();
            if !create_untypeds_for_region(root_cnode_cap, false, reg, first_untyped_slot) {
                debug!(
                    "ERROR: creation of untypeds for free memory region :{} at
                [{}..{}] failed\n",
                    i,
                    reg.start.raw(),
                    reg.end.raw()
                );
            }
        }
        (*ndks_boot.bi_frame).untyped = SlotRegion {
            start: first_untyped_slot,
            end: ndks_boot.slot_pos_cur,
        };
        true
    }
}

fn create_untypeds_for_region(
    root_cnode_cap: &cap_cnode_cap,
    device_memory: bool,
    mut reg: Region,
    first_untyped_slot: seL4_SlotPos,
) -> bool {
    while !reg.is_empty() {
        let mut size_bits =
            SEL4_WORD_BITS - 1 - (reg.end.raw() - reg.start.raw()).leading_zeros() as usize;
        if size_bits > MAX_UNTYPED_BITS {
            size_bits = MAX_UNTYPED_BITS;
        }
        if !reg.start.is_null() {
            let align_bits = reg.start.raw().trailing_zeros() as usize;
            if size_bits > align_bits {
                size_bits = align_bits;
            }
        }
        if size_bits >= SEL4_MIN_UNTYPED_BITS {
            if !provide_untyped_cap(
                root_cnode_cap,
                device_memory,
                reg.start.raw(),
                size_bits,
                first_untyped_slot,
            ) {
                return false;
            }
        }
        reg.start += bit!(size_bits);
    }
    return true;
}

fn provide_untyped_cap(
    root_cnode_cap: &cap_cnode_cap,
    device_memory: bool,
    pptr: usize,
    size_bits: usize,
    first_untyped_slot: seL4_SlotPos,
) -> bool {
    if size_bits > MAX_UNTYPED_BITS || size_bits < SEL4_MIN_UNTYPED_BITS {
        debug!("Kernel init: Invalid untyped size {}", size_bits);
        return false;
    }

    if !is_aligned!(pptr, size_bits) {
        debug!(
            "Kernel init: Unaligned untyped pptr {} (alignment {})",
            pptr, size_bits
        );
        return false;
    }

    if !device_memory && !pptr_in_kernel_window(pptr) {
        debug!(
            "Kernel init: Non-device untyped pptr {:#x} outside kernel window",
            pptr
        );
        return false;
    }

    if !device_memory && !pptr_in_kernel_window(pptr + mask_bits!(size_bits)) {
        debug!(
            "Kernel init: End of non-device untyped at {} outside kernel window (size {})",
            pptr, size_bits
        );
        return false;
    }
    let ret: bool;
    unsafe {
        let i = ndks_boot.slot_pos_cur - first_untyped_slot;
        if i < CONFIG_MAX_NUM_BOOTINFO_UNTYPED_CAPS {
            (*ndks_boot.bi_frame).untypedList[i] = UntypedDesc {
                paddr: pptr!(pptr).to_paddr(),
                sizeBits: size_bits as u8,
                isDevice: device_memory as u8,
                padding: [0; 6],
            };
            let ut_cap = cap_untyped_cap::new(
                max_free_index(size_bits) as u64,
                device_memory as u64,
                size_bits as u64,
                pptr as u64,
            );
            ret = provide_cap(root_cnode_cap, ut_cap.unsplay().clone());
        } else {
            debug!("Kernel init: Too many untyped regions for boot info");
            ret = true
        }
    }
    ret
}
