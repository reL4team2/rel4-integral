use rel4_arch::basic::PPtr;
use sel4_common::{
    arch::{vm_rights_t, ObjectType},
    sel4_config::ASID_INVALID,
    structures_gen::{cap, cap_frame_cap, cap_page_table_cap},
};

pub fn arch_create_object(
    obj_type: ObjectType,
    region_base: PPtr,
    user_size: usize,
    device_mem: usize,
) -> cap {
    match obj_type {
        ObjectType::PageTableObject => {
            cap_page_table_cap::new(ASID_INVALID as u64, region_base.as_u64(), 0, 0).unsplay()
        }

        ObjectType::NormalPageObject | ObjectType::GigaPageObject | ObjectType::MegaPageObject => {
            cap_frame_cap::new(
                ASID_INVALID as u64,
                region_base.as_u64(),
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
                region_base.raw(),
                region_base.raw() + user_size
            )
        }
    }
}
