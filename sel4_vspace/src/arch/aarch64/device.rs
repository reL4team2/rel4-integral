use super::boot::map_kernel_frame;
use crate::{paddr_t, pptr_t, vm_attributes_t};
use sel4_common::arch::vm_rights_t::VMKernelOnly;
use sel4_common::{sel4_config::PAGE_BITS, BIT};

// TODO: support hypervisor mode
pub const KDEV_BASE: usize = 0xffffffffffe00000;
pub(crate) const NUM_KERNEL_DEVICE_FRAMES: usize = 3;
pub(crate) const UART_PPTR: usize = KDEV_BASE + 0x0;
pub(crate) const GIC_V2_DISTRIBUTOR_PPTR: usize = KDEV_BASE + 0x1000;
pub(crate) const GIC_V2_CONTROLLER_PPTR: usize = KDEV_BASE + 0x2000;
#[derive(Copy, Clone)]
struct kernel_frame_t {
    paddr: paddr_t,
    pptr: pptr_t,
    armExecuteNever: isize,
    userAvailable: isize,
}

#[derive(Copy, Clone)]
#[repr(C)]
struct p_region_t {
    pub start: usize,
    pub end: usize,
}
extern "C" {
    pub(self) fn reserve_region(reg: p_region_t) -> bool;
}

#[no_mangle]
#[link_section = ".boot.text"]
pub(self) static mut kernel_device_frames: [kernel_frame_t; NUM_KERNEL_DEVICE_FRAMES] = [
    kernel_frame_t {
        paddr: paddr_t(0x9000000),
        pptr: UART_PPTR,
        armExecuteNever: 1,
        userAvailable: 1,
    },
    kernel_frame_t {
        paddr: paddr_t(0x8000000),
        pptr: GIC_V2_DISTRIBUTOR_PPTR,
        armExecuteNever: 1,
        userAvailable: 0,
    },
    kernel_frame_t {
        paddr: paddr_t(0x8010000),
        pptr: GIC_V2_CONTROLLER_PPTR,
        armExecuteNever: 1,
        userAvailable: 0,
    },
];
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
