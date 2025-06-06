use sel4_common::structures_gen::cap;
use sel4_common::{
    arch::{vm_rights_t, ObjectType},
    sel4_config::{ARM_HUGE_PAGE, ARM_LARGE_PAGE, ARM_SMALL_PAGE, ASID_INVALID},
    structures_gen::{cap_frame_cap, cap_page_table_cap, cap_vspace_cap},
};
use sel4_vspace::pptr_t;

pub fn arch_create_object(
    obj_type: ObjectType,
    region_base: pptr_t,
    user_size: usize,
    device_mem: usize,
) -> cap {
    match obj_type {
        ObjectType::seL4_ARM_SmallPageObject => cap_frame_cap::new(
            ASID_INVALID as u64,
            region_base as u64,
            ARM_SMALL_PAGE as u64,
            0,
            vm_rights_t::VMReadWrite as _,
            device_mem as u64,
        )
        .unsplay(),
        ObjectType::seL4_ARM_LargePageObject => cap_frame_cap::new(
            ASID_INVALID as u64,
            region_base as u64,
            ARM_LARGE_PAGE as u64,
            0,
            vm_rights_t::VMReadWrite as _,
            device_mem as u64,
        )
        .unsplay(),
        ObjectType::seL4_ARM_HugePageObject => cap_frame_cap::new(
            ASID_INVALID as u64,
            region_base as u64,
            ARM_HUGE_PAGE as u64,
            0,
            vm_rights_t::VMReadWrite as _,
            device_mem as u64,
        )
        .unsplay(),
        ObjectType::seL4_ARM_VSpaceObject => {
            cap_vspace_cap::new(ASID_INVALID as u64, region_base as u64, 0).unsplay()
        }
        ObjectType::seL4_ARM_PageTableObject => {
            cap_page_table_cap::new(ASID_INVALID as u64, region_base as u64, 0, 0).unsplay()
        }
        _ => {
            unimplemented!(
                "create object: {:?} region: {:#x} - {:#x}",
                obj_type,
                region_base,
                region_base + user_size
            )
        }
    }
}
