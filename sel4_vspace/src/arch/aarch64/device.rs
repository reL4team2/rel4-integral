use super::boot::map_kernel_frame;
use crate::vm_attributes_t;
use rel4_arch::basic::PRegion;
use rel4_arch::paddr;
use sel4_common::arch::vm_rights_t::VMKernelOnly;
use sel4_common::platform::kernel_device_frames;
use sel4_common::sel4_config::PAGE_BITS;

extern "C" {
    pub(self) fn reserve_region(reg: PRegion) -> bool;
}

#[no_mangle]
pub fn map_kernel_devices() {
    unsafe {
        for kernel_frame in kernel_device_frames {
            let vm_attr: vm_attributes_t = vm_attributes_t(kernel_frame.armExecuteNever as usize);
            map_kernel_frame(
                kernel_frame.paddr.raw(),
                kernel_frame.pptr.raw(),
                VMKernelOnly,
                vm_attr,
            );
            if kernel_frame.userAvailable == 0 {
                reserve_region(PRegion::new(
                    paddr!(kernel_frame.paddr.raw()),
                    paddr!(kernel_frame.paddr.raw() + bit!(PAGE_BITS)),
                ));
            }
        }
    }
}
