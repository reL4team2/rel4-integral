use sel4_common::{
    arch::maskVMRights, cap_rights::seL4_CapRights_t, structures::exception_t,
    structures_gen::cap_tag, utils::pageBitsForSize, vm_rights::vm_rights_from_word, MASK,
};

use crate::{
    capability::{cap_arch_func, zombie::zombie_func},
    cte::{cte_t, deriveCap_ret},
};
use sel4_common::structures_gen::{cap, cap_null_cap};

impl cap_arch_func for cap {
    fn get_cap_ptr(&self) -> usize {
        match self.get_tag() {
            cap_tag::cap_untyped_cap => cap::cap_untyped_cap(self).get_capPtr() as usize,
            cap_tag::cap_endpoint_cap => cap::cap_endpoint_cap(self).get_capEPPtr() as usize,
            cap_tag::cap_notification_cap => {
                cap::cap_notification_cap(self).get_capNtfnPtr() as usize
            }
            cap_tag::cap_cnode_cap => cap::cap_cnode_cap(self).get_capCNodePtr() as usize,
            cap_tag::cap_thread_cap => cap::cap_thread_cap(self).get_capTCBPtr() as usize,
            cap_tag::cap_zombie_cap => cap::cap_zombie_cap(self).get_zombie_ptr() as usize,
            cap_tag::cap_frame_cap => cap::cap_frame_cap(self).get_capFBasePtr() as usize,
            cap_tag::cap_page_table_cap => {
                cap::cap_page_table_cap(self).get_capPTBasePtr() as usize
            }
            cap_tag::cap_vspace_cap => cap::cap_vspace_cap(self).get_capVSBasePtr() as usize,
            // cap_tag::CapPageDirectoryCap => self.get_pd_base_ptr(),
            // cap_tag::CapPageUpperDirectoryCap => self.get_pud_base_ptr(),
            // cap_tag::CapPageGlobalDirectoryCap => self.get_pgd_base_ptr(),
            cap_tag::cap_asid_control_cap => 0,
            cap_tag::cap_asid_pool_cap => cap::cap_asid_pool_cap(self).get_capASIDPool() as usize,
            _ => 0,
        }
    }

    #[inline]
    fn is_vtable_root(&self) -> bool {
        self.get_tag() == cap_tag::cap_vspace_cap
    }

    #[inline]
    fn is_valid_native_root(&self) -> bool {
        self.is_vtable_root() && cap::cap_vspace_cap(self).get_capVSIsMapped() != 0
    }

    #[inline]
    fn is_valid_vtable_root(&self) -> bool {
        self.is_valid_native_root()
    }
}

impl cte_t {
    pub fn arch_derive_cap(&self, capability: &cap) -> deriveCap_ret {
        let mut ret = deriveCap_ret {
            status: exception_t::EXCEPTION_NONE,
            capability: cap_null_cap::new().unsplay(),
        };
        match capability.get_tag() {
            // cap_tag::CapPageGlobalDirectoryCap => {
            //     if cap.get_pgd_is_mapped() != 0 {
            //         ret.cap = cap.clone();
            //         ret.status = exception_t::EXCEPTION_NONE;
            //     } else {
            //         ret.cap = cap_t::new_null_cap();
            //         ret.status = exception_t::EXCEPTION_SYSCALL_ERROR;
            //     }
            // }
            // cap_tag::CapPageUpperDirectoryCap => {
            //     if cap.get_pud_is_mapped() != 0 {
            //         ret.cap = cap.clone();
            //         ret.status = exception_t::EXCEPTION_NONE;
            //     } else {
            //         ret.cap = cap_t::new_null_cap();
            //         ret.status = exception_t::EXCEPTION_SYSCALL_ERROR;
            //     }
            // }
            // cap_tag::CapPageDirectoryCap => {
            //     if cap.get_pud_is_mapped() != 0 {
            //         ret.cap = cap.clone();
            //         ret.status = exception_t::EXCEPTION_NONE;
            //     } else {
            //         ret.cap = cap_t::new_null_cap();
            //         ret.status = exception_t::EXCEPTION_SYSCALL_ERROR;
            //     }
            // }
            cap_tag::cap_vspace_cap => {
                if cap::cap_vspace_cap(capability).get_capVSIsMapped() != 0 {
                    ret.capability = capability.clone();
                    ret.status = exception_t::EXCEPTION_NONE;
                } else {
                    ret.capability = cap_null_cap::new().unsplay();
                    ret.status = exception_t::EXCEPTION_SYSCALL_ERROR;
                }
            }
            cap_tag::cap_page_table_cap => {
                if cap::cap_page_table_cap(capability).get_capPTIsMapped() != 0 {
                    ret.capability = capability.clone();
                    ret.status = exception_t::EXCEPTION_NONE;
                } else {
                    ret.capability = cap_null_cap::new().unsplay();
                    ret.status = exception_t::EXCEPTION_SYSCALL_ERROR;
                }
            }
            cap_tag::cap_frame_cap => {
                let newCap = capability.clone();
                cap::cap_frame_cap(&newCap).set_capFMappedASID(0);
                ret.capability = newCap;
            }
            cap_tag::cap_asid_control_cap | cap_tag::cap_asid_pool_cap => {
                ret.capability = capability.clone();
            }
            _ => {
                panic!(" Invalid arch cap type : {}", capability.get_tag() as usize);
            }
        }
        ret
    }
}

pub fn arch_mask_cap_rights(rights: seL4_CapRights_t, capability: &cap) -> cap {
    match capability.get_tag() {
        cap_tag::cap_frame_cap => {
            let mut vm_rights =
                vm_rights_from_word(cap::cap_frame_cap(capability).get_capFVMRights() as usize);
            vm_rights = maskVMRights(vm_rights, rights);
            let new_cap = capability.clone();
            cap::cap_frame_cap(&new_cap).set_capFVMRights(vm_rights as u64);
            new_cap
        }
        _ => capability.clone(),
    }
}

pub fn arch_same_region_as(cap1: &cap, cap2: &cap) -> bool {
    match cap1.get_tag() {
        cap_tag::cap_frame_cap => {
            if cap2.get_tag() == cap_tag::cap_frame_cap {
                let botA = cap::cap_frame_cap(cap1).get_capFBasePtr() as usize;
                let botB = cap::cap_frame_cap(cap2).get_capFBasePtr() as usize;
                let topA = botA
                    + MASK!(pageBitsForSize(
                        cap::cap_frame_cap(cap1).get_capFSize() as usize
                    ));
                let topB = botB
                    + MASK!(pageBitsForSize(
                        cap::cap_frame_cap(cap2).get_capFSize() as usize
                    ));
                return (botA <= botB) && (topA >= topB) && (botB <= topB);
            }
        }
        cap_tag::cap_page_table_cap => {
            if cap2.get_tag() == cap_tag::cap_page_table_cap {
                return cap::cap_page_table_cap(cap1).get_capPTBasePtr()
                    == cap::cap_page_table_cap(cap2).get_capPTBasePtr();
            }
        }
        cap_tag::cap_vspace_cap => {
            if cap2.get_tag() == cap_tag::cap_vspace_cap {
                return cap::cap_vspace_cap(cap1).get_capVSBasePtr()
                    == cap::cap_vspace_cap(cap2).get_capVSBasePtr();
            }
        }
        cap_tag::cap_asid_control_cap => {
            return cap2.get_tag() == cap_tag::cap_asid_control_cap;
        }
        cap_tag::cap_asid_pool_cap => {
            if cap2.get_tag() == cap_tag::cap_asid_pool_cap {
                return cap::cap_asid_pool_cap(cap1).get_capASIDPool()
                    == cap::cap_asid_pool_cap(cap2).get_capASIDPool();
            }
        }
        _ => panic!("unknown cap"),
    }
    false
}

pub fn arch_same_object_as(cap1: &cap, cap2: &cap) -> bool {
    if cap1.get_tag() == cap_tag::cap_frame_cap && cap2.get_tag() == cap_tag::cap_frame_cap {
        return cap::cap_frame_cap(cap1).get_capFBasePtr()
            == cap::cap_frame_cap(cap2).get_capFBasePtr()
            && cap::cap_frame_cap(cap1).get_capFSize() == cap::cap_frame_cap(cap2).get_capFSize()
            && cap::cap_frame_cap(cap1).get_capFIsDevice()
                == cap::cap_frame_cap(cap2).get_capFIsDevice();
    }
    arch_same_region_as(cap1, cap2)
}
