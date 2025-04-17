use super::boot::map_kernel_frame;
use crate::vm_attributes_t;
use sel4_common::arch::vm_rights_t::VMKernelOnly;
use sel4_common::platform::kernel_device_frames;
use sel4_common::structures::p_region_t;
use sel4_common::{sel4_config::PAGE_BITS, BIT};

extern "C" {
    pub(self) fn reserve_region(reg: p_region_t) -> bool;
}

#[no_mangle]
pub fn map_kernel_devices() {
    unsafe {
        for kernel_frame in kernel_device_frames {
            let vm_attr: vm_attributes_t = vm_attributes_t(kernel_frame.armExecuteNever as usize);
            map_kernel_frame(
                kernel_frame.paddr.0,
                kernel_frame.pptr,
                VMKernelOnly,
                vm_attr,
            );
            if kernel_frame.userAvailable == 0 {
                reserve_region(p_region_t {
                    start: kernel_frame.paddr.0,
                    end: kernel_frame.paddr.0 + BIT!(PAGE_BITS),
                });
            }
        }
    }
}
