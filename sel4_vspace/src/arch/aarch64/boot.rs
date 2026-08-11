use rel4_arch::basic::{PPtr, VPtr};
use sel4_common::{
    arch::{
        config::{PADDR_BASE, PADDR_TOP, PPTR_BASE, PPTR_TOP},
        vm_rights_t,
    },
    sel4_config::{ARM_LARGE_PAGE, ARM_SMALL_PAGE, PUD_INDEX_BITS, SEL4_LARGE_PAGE_BITS},
    structures_gen::{cap, cap_frame_cap, cap_page_table_cap, cap_vspace_cap},
    utils::convert_to_mut_type_ref,
};

use crate::{
    arch::VAddr, asid_t, get_kernel_page_directory_base_by_index, get_kernel_page_table_base,
    get_kernel_page_upper_directory_base, kpptr_to_paddr, mair_types,
    set_kernel_page_directory_by_index, set_kernel_page_global_directory_by_index,
    set_kernel_page_table_by_index, set_kernel_page_upper_directory_by_index, vm_attributes_t, PTE,
};

use super::{map_kernel_devices, page_slice, vspace_root_slice};

#[derive(PartialEq, Eq, Debug)]
enum find_type {
    PDE,
    PUDE,
    PTE,
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn rust_map_kernel_window() {
    set_kernel_page_global_directory_by_index(
        (VAddr(PPTR_BASE)).get_kpt_index(0),
        PTE::pte_new_table(kpptr_to_paddr(get_kernel_page_upper_directory_base())),
    );

    let mut idx = VAddr(PPTR_BASE).get_kpt_index(1);
    while idx < VAddr(PPTR_TOP).get_kpt_index(1) {
        set_kernel_page_upper_directory_by_index(
            idx,
            PTE::pte_new_table(kpptr_to_paddr(get_kernel_page_directory_base_by_index(idx))),
        );
        idx += 1;
    }

    let mut vaddr = PPTR_BASE;
    let mut paddr = PADDR_BASE;
    let shareable = if cfg!(feature = "enable_smp") { 3 } else { 0 };

    while paddr < PADDR_TOP {
        #[cfg(feature = "hypervisor")]
        set_kernel_page_directory_by_index(
            VAddr(vaddr).get_kpt_index(1),
            VAddr(vaddr).get_kpt_index(2),
            PTE::pte_new_page(
                0,
                paddr!(paddr),
                0,
                1,
                shareable,
                0,
                mair_types::NORMAL as usize,
            ),
        );
        #[cfg(not(feature = "hypervisor"))]
        set_kernel_page_directory_by_index(
            VAddr(vaddr).get_kpt_index(1),
            VAddr(vaddr).get_kpt_index(2),
            PTE::pte_new_page(
                1,
                paddr!(paddr),
                0,
                1,
                shareable,
                0,
                mair_types::NORMAL as usize,
            ),
        );

        vaddr += bit!(SEL4_LARGE_PAGE_BITS);
        paddr += bit!(SEL4_LARGE_PAGE_BITS)
    }

    set_kernel_page_upper_directory_by_index(
        VAddr(PPTR_TOP).get_kpt_index(1),
        PTE::pte_new_table(kpptr_to_paddr(get_kernel_page_directory_base_by_index(
            bit!(PUD_INDEX_BITS) - 1,
        ))),
    );
    set_kernel_page_directory_by_index(
        bit!(PUD_INDEX_BITS) - 1,
        bit!(PUD_INDEX_BITS) - 1,
        PTE::pte_new_table(kpptr_to_paddr(get_kernel_page_table_base())),
    );
    map_kernel_devices();
    // ffi_call!(map_kernel_devices());
}

#[no_mangle]
pub fn map_kernel_frame(
    paddr: usize,
    vaddr: usize,
    vm_rights: vm_rights_t,
    attributes: vm_attributes_t,
) {
    #[cfg(feature = "hypervisor")]
    let uxn = attributes.get_arm_execute_never() as usize;
    #[cfg(not(feature = "hypervisor"))]
    let uxn = 1; /* unprivileged execute never */
    let attr_index: usize;
    let shareable: usize;
    if attributes.get_page_cacheable() != 0 {
        attr_index = mair_types::NORMAL as usize;
        shareable = if cfg!(feature = "enable_smp") { 3 } else { 0 };
    } else {
        attr_index = mair_types::DEVICE_nGnRnE as usize;
        shareable = 0;
    }
    set_kernel_page_table_by_index(
        VAddr(vaddr).get_kpt_index(3),
        PTE::pte_new_4k_page(
            uxn,
            paddr!(paddr),
            0,
            1,
            shareable,
            PTE::ap_from_vm_rights_t(vm_rights),
            attr_index,
        ),
    );
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn map_it_pt_cap(vspace_cap: &cap_vspace_cap, pt_cap: &cap_page_table_cap) {
    let vspace_root = vspace_cap.clone().get_capVSBasePtr() as usize;
    let vptr = pt_cap.get_capPTMappedAddress() as usize;
    let pt = pt_cap.get_capPTBasePtr() as usize;
    let target_pte =
        convert_to_mut_type_ref::<PTE>(find_pt(vspace_root, vptr.into(), find_type::PDE));
    target_pte.set_next_level_paddr(pptr!(pt).to_paddr());
    // TODO: move 0x3 into a proper position.
    target_pte.set_attr(3);
}

/// When hypervisor support is enabled (AARCH64_VSPACE_S2_START_L1), vspaceRoot
/// is directly the PUD level — skip PGD lookup.
/// Now the we assume hypervisor =  AARCH64_VSPACE_S2_START_L1, because I don't want to add more features like sel4 kernel. It is so complicated in rust.
#[no_mangle]
#[link_section = ".boot.text"]
pub fn map_it_pd_cap(vspace_cap: &cap_vspace_cap, pd_cap: &cap_page_table_cap) {
    let pd_addr = pd_cap.get_capPTBasePtr() as usize;
    let vptr: VAddr = (pd_cap.get_capPTMappedAddress() as usize).into();
    assert_eq!(pd_cap.get_capPTIsMapped(), 1);
    #[cfg(feature = "hypervisor")]
    {
        // SL0=1: root table is PUD with 1024 entries, write directly
        let pud = vspace_root_slice::<PTE>(pptr!(vspace_cap.get_capVSBasePtr()));
        pud[vptr.pud_index()] = PTE::new_page(pptr!(pd_addr).to_paddr(), 0x3);
    }
    #[cfg(not(feature = "hypervisor"))]
    {
        let pgd = page_slice::<PTE>(pptr!(vspace_cap.get_capVSBasePtr()));
        assert_eq!(pgd[vptr.pgd_index()].attr(), 0x3);
        let pud = pgd[vptr.pgd_index()].next_level_slice::<PTE>();
        pud[vptr.pud_index()] = PTE::new_page(pptr!(pd_addr).to_paddr(), 0x3);
    }
}

/// When hypervisor support is enabled, this function is unused
/// (PUD creation is skipped entirely).
#[cfg(not(feature = "hypervisor"))]
pub fn map_it_pud_cap(vspace_cap: &cap_vspace_cap, pud_cap: &cap_page_table_cap) {
    let pgd = page_slice::<PTE>(pptr!(vspace_cap.get_capVSBasePtr()));
    let pud_addr = pud_cap.get_capPTBasePtr() as usize;
    let vptr: VAddr = (pud_cap.get_capPTMappedAddress() as usize).into();
    assert_eq!(pud_cap.get_capPTIsMapped(), 1);
    pgd[vptr.pgd_index()] = PTE::new_page(pptr!(pud_addr).to_paddr(), 0x3);
}

/// TODO: Write the comments.
#[no_mangle]
#[link_section = ".boot.text"]
pub fn map_it_frame_cap(vspace_cap: &cap_vspace_cap, frame_cap: &cap_frame_cap, exec: bool) {
    let pte = convert_to_mut_type_ref::<PTE>(find_pt(
        vspace_cap.get_capVSBasePtr() as usize,
        (frame_cap.get_capFMappedAddress() as usize).into(),
        find_type::PTE,
    ));
    // TODO: Make set_attr usage more efficient.
    // TIPS: exec true will be cast to 1 and false to 0.
    let shareable = if cfg!(feature = "enable_smp") { 3 } else { 0 };
    #[cfg(not(feature = "hypervisor"))]
    let (ng, attr) = (1, mair_types::NORMAL as usize);
    #[cfg(feature = "hypervisor")]
    let (ng, attr) = (0, mair_types::S2_NORMAL as usize);
    // AP=1 for the initial thread frame caps (VMReadWrite in Stage-1, S2AP=01? no: Stage-2 uses S2AP=11 for RW)
    // But this is boot code using hardcoded AP=1. In C kernel, APFromVMRights(VMReadWrite) returns 3 for hypervisor.
    // However, map_it_frame_cap hardcodes AP=1 (VMReadWrite in S2AP=01 = Read-only!)
    // This is a bug in the original Rust code. Let's fix with correct S2AP.
    #[cfg(feature = "hypervisor")]
    let ap = PTE::ap_from_vm_rights_t(vm_rights_t::VMReadWrite); // S2AP=11
    #[cfg(not(feature = "hypervisor"))]
    let ap = 1; // AP[2:1]=01: EL0 RW
    pte.set_attr(PTE::pte_new_4k_page((!exec) as usize, paddr!(0), ng, 1, shareable, ap, attr).0);
    pte.set_next_level_paddr(pptr!(frame_cap.get_capFBasePtr()).to_paddr());
}

/// TODO: Write the comments.
#[link_section = ".boot.text"]
#[cfg(not(feature = "hypervisor"))]
fn find_pt(vspace_root: usize, vptr: VAddr, ftype: find_type) -> usize {
    let pgd = page_slice::<PTE>(pptr!(vspace_root));
    let pud = pgd[vptr.pgd_index()].next_level_slice::<PTE>();
    if ftype == find_type::PUDE {
        return pud[vptr.pud_index()].self_addr();
    }
    let pd = pud[vptr.pud_index()].next_level_slice::<PTE>();
    if ftype == find_type::PDE {
        return pd[vptr.pd_index()].self_addr();
    }
    let pt = pd[vptr.pd_index()].next_level_slice::<PTE>();
    assert_eq!(ftype, find_type::PTE);
    pt[vptr.pt_index()].self_addr()
}

/// Hypervisor (SL0=1) variant: vspaceRoot IS the PUD (1024 entries), skip PGD lookup.
#[link_section = ".boot.text"]
#[cfg(feature = "hypervisor")]
fn find_pt(vspace_root: usize, vptr: VAddr, ftype: find_type) -> usize {
    let pud = vspace_root_slice::<PTE>(pptr!(vspace_root));
    let pd = pud[vptr.pud_index()].next_level_slice::<PTE>();
    if ftype == find_type::PDE {
        return pd[vptr.pd_index()].self_addr();
    }
    let pt = pd[vptr.pd_index()].next_level_slice::<PTE>();
    assert_eq!(ftype, find_type::PTE);
    pt[vptr.pt_index()].self_addr()
}

/// Create a new pud cap in the vspace.
/// Only used in non-hypervisor mode (4-level page table).
/// hypervisor mode only support 3-level 40 bit page table
///
/// vptr is the virtual address of the pud cap will be created
/// pptr is the address to the physical address will be mapped
#[cfg(not(feature = "hypervisor"))]
#[no_mangle]
#[link_section = ".boot.text"]
pub fn create_it_pud_cap(
    vspace_cap: &cap_vspace_cap,
    pptr: PPtr,
    vptr: VPtr,
    asid: usize,
) -> cap_page_table_cap {
    let capability = cap_page_table_cap::new(asid as u64, pptr.raw() as u64, 1, vptr.raw() as u64);
    map_it_pud_cap(vspace_cap, &capability);
    return capability;
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn create_it_pd_cap(vspace_cap: &cap_vspace_cap, pptr: PPtr, vptr: VPtr, asid: usize) -> cap {
    let capability = cap_page_table_cap::new(asid as u64, pptr.raw() as u64, 1, vptr.raw() as u64);
    map_it_pd_cap(vspace_cap, &capability);
    capability.unsplay()
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn create_unmapped_it_frame_cap(pptr: PPtr, use_large: bool) -> cap_frame_cap {
    create_it_frame_cap(pptr, vptr!(0), 0, use_large)
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn create_it_frame_cap(pptr: PPtr, vptr: VPtr, asid: asid_t, use_large: bool) -> cap_frame_cap {
    let frame_size;
    if use_large {
        frame_size = ARM_LARGE_PAGE;
    } else {
        frame_size = ARM_SMALL_PAGE;
    }
    cap_frame_cap::new(
        asid as u64,
        pptr.raw() as u64,
        frame_size as u64,
        vptr.raw() as u64,
        vm_rights_t::VMReadWrite as u64,
        0,
    )
}

#[no_mangle]
pub fn create_mapped_it_frame_cap(
    pd_cap: &cap_vspace_cap,
    pptr: PPtr,
    vptr: VPtr,
    asid: usize,
    use_large: bool,
    exec: bool,
) -> cap_frame_cap {
    let capability = create_it_frame_cap(pptr, vptr, asid, use_large);
    map_it_frame_cap(pd_cap, &capability, exec);
    capability
}