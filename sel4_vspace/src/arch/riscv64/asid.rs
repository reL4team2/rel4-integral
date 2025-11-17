use core::{arch::asm, intrinsics::unlikely};

use rel4_arch::basic::PPtr;
use sel4_common::{
    sel4_config::{ASID_HIGH_BITS, ASID_LOW_BITS, IT_ASID},
    structures::exception_t,
    structures_gen::{
        cap, cap_asid_pool_cap, cap_page_table_cap, lookup_fault, lookup_fault_invalid_root,
    },
    utils::convert_to_option_mut_type_ref,
};

use crate::{asid_pool_t, asid_t, findVSpaceForASID_ret, set_vm_root, PTE};

///存放`asid pool`的数组，每一个下标对应一个`asid pool`，
///一个`asid pool`可以存放`ASID_LOW_BITS`个asid值
#[no_mangle]
pub static mut riscvKSASIDTable: [*mut asid_pool_t; bit!(ASID_HIGH_BITS)] =
    [0 as *mut asid_pool_t; bit!(ASID_HIGH_BITS)];

pub fn write_it_asid_pool(it_ap_cap: &cap_asid_pool_cap, it_lvl1pt_cap: &cap_page_table_cap) {
    let ap = it_ap_cap.get_capASIDPool() as usize;
    unsafe {
        let ptr = (ap + 8 * IT_ASID) as *mut usize;
        *ptr = it_lvl1pt_cap.get_capPTBasePtr() as usize;
        riscvKSASIDTable[IT_ASID >> ASID_LOW_BITS] = ap as *mut asid_pool_t;
    }
}

///在`asid pool`中删除对应的`asid`,
/// 并设置新使用的页表为`default_vspace_cap`提供的页表
///
/// delete the asid from asid pool.
pub fn delete_asid(
    asid: asid_t,
    vspace: *mut PTE,
    default_vspace_cap: &cap,
) -> Result<(), lookup_fault> {
    unsafe {
        let poolPtr = riscvKSASIDTable[asid >> ASID_LOW_BITS];
        if poolPtr as usize != 0 && (*poolPtr).array[asid & mask_bits!(ASID_LOW_BITS)] == vspace {
            #[cfg(target_arch = "riscv64")]
            hw_asid_flush(asid);
            (*poolPtr).array[asid & mask_bits!(ASID_LOW_BITS)] = 0 as *mut PTE;
            set_vm_root(&default_vspace_cap)
        } else {
            Ok(())
        }
    }
}

/// `riscvKSASIDSpace`设置对应`index`的`asid pool`
///
/// From `riscvKSASIDSpace` set the index-relevant asid pool.
pub fn set_asid_pool_by_index(index: usize, pool_ptr: PPtr) {
    // assert!(index < bit!(ASID_HIGH_BITS));
    unsafe {
        riscvKSASIDTable[index] = pool_ptr.get_mut_ptr();
    }
}

/// `riscvKSASIDSpace`寻找对应`index`的`asid pool`
///
/// From `riscvKSASIDSpace` get the index-relevant asid pool.
#[inline]
pub fn get_asid_pool_by_index(index: usize) -> Option<&'static mut asid_pool_t> {
    unsafe {
        if unlikely(index >= bit!(ASID_HIGH_BITS)) {
            return None;
        }
        return convert_to_option_mut_type_ref::<asid_pool_t>(riscvKSASIDTable[index] as usize);
    }
}

///根据给定的`asid`在`riscvKSASIDTable`中寻找对应的虚拟地址空间页表基址
///
/// Find the root page table associated with asid.
#[no_mangle]
pub fn find_vspace_for_asid(asid: asid_t) -> findVSpaceForASID_ret {
    let mut ret: findVSpaceForASID_ret = findVSpaceForASID_ret {
        status: exception_t::EXCEPTION_FAULT,
        vspace_root: None,
        lookup_fault: None,
    };

    let poolPtr = unsafe { riscvKSASIDTable[asid >> ASID_LOW_BITS] };
    if poolPtr as usize == 0 {
        ret.lookup_fault = Some(lookup_fault_invalid_root::new().unsplay());
        ret.vspace_root = None;
        ret.status = exception_t::EXCEPTION_LOOKUP_FAULT;
        return ret;
    }
    let vspace_root = unsafe { (*poolPtr).array[asid & mask_bits!(ASID_LOW_BITS)] };
    if vspace_root as usize == 0 {
        ret.lookup_fault = Some(lookup_fault_invalid_root::new().unsplay());
        ret.vspace_root = None;
        ret.status = exception_t::EXCEPTION_LOOKUP_FAULT;
        return ret;
    }
    ret.vspace_root = Some(vspace_root);
    ret.status = exception_t::EXCEPTION_NONE;
    // vspace_root0xffffffc17fec1000
    return ret;
}

///在`riscvKSASIDTable`中删除对应的`asid pool`，
/// 并设置新使用的页表为`default_vspace_cap`提供的页表
///
/// delete the asid pool which contains many asids.
pub fn delete_asid_pool(
    asid_base: asid_t,
    pool: *mut asid_pool_t,
    default_vspace_cap: &cap,
) -> Result<(), lookup_fault> {
    unsafe {
        if riscvKSASIDTable[asid_base >> ASID_LOW_BITS] == pool {
            riscvKSASIDTable[asid_base >> ASID_LOW_BITS] = 0 as *mut asid_pool_t;
            set_vm_root(default_vspace_cap)
        } else {
            Ok(())
        }
    }
}

///清除`TLB`中对应`asid`的项
#[inline]
pub fn hw_asid_flush(asid: asid_t) {
    unsafe {
        asm!("sfence.vma x0, {0}",in(reg) asid);
    }
}
