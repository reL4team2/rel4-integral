use crate::shared_types_bf_gen::seL4_CapRights;

#[repr(usize)]
#[derive(PartialEq, Eq)]
pub enum vm_rights_t {
    VMKernelOnly = 0,
    VMReadWrite = 1,
    VMReadOnly = 3,
}

pub fn maskVMRights(vm_rights: vm_rights_t, rights: seL4_CapRights) -> vm_rights_t {
    if vm_rights == vm_rights_t::VMReadOnly && rights.get_capAllowRead() != 0 {
        return vm_rights_t::VMReadOnly;
    }
    if vm_rights == vm_rights_t::VMReadWrite && rights.get_capAllowRead() != 0 {
        if rights.get_capAllowWrite() != 0 {
            return vm_rights_t::VMReadWrite;
        } else {
            return vm_rights_t::VMReadOnly;
        }
    }
    vm_rights_t::VMKernelOnly
}
