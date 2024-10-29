use sel4_common::{
    arch::{vm_rights_t, ObjectType},
    sel4_config::asidInvalid,
    structures_gen::{cap, cap_frame_cap, cap_page_table_cap},
};
use sel4_vspace::pptr_t;

pub fn arch_create_object(
    obj_type: ObjectType,
    region_base: pptr_t,
    user_size: usize,
    device_mem: usize,
) -> cap {
    match obj_type {
        ObjectType::PageTableObject => {
            cap_page_table_cap::new(asidInvalid as u64, region_base as u64, 0, 0).unsplay()
        }

        ObjectType::NormalPageObject | ObjectType::GigaPageObject | ObjectType::MegaPageObject => {
            cap_frame_cap::new(
                asidInvalid as u64,
                region_base as u64,
                obj_type.get_frame_type() as u64,
                vm_rights_t::VMReadWrite as u64,
                device_mem as u64,
                0,
            )
            .unsplay()
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
