use sel4_common::{
    arch::{
        config::{PADDR_BASE, PADDR_TOP, PPTR_BASE, PPTR_TOP},
        vm_rights_t,
    },
    sel4_config::{seL4_LargePageBits, ARM_Large_Page, ARM_Small_Page, PUD_INDEX_BITS},
    structures_gen::{cap, cap_frame_cap, cap_page_table_cap, cap_vspace_cap},
    utils::convert_to_mut_type_ref,
    BIT,
};

use crate::{
    arch::VAddr, asid_t, get_kernel_page_directory_base_by_index, get_kernel_page_table_base,
    get_kernel_page_upper_directory_base, kpptr_to_paddr, mair_types, pptr_t, pptr_to_paddr,
    set_kernel_page_directory_by_index, set_kernel_page_global_directory_by_index,
    set_kernel_page_table_by_index, set_kernel_page_upper_directory_by_index, vm_attributes_t,
    vptr_t, PTE,
};

use super::{map_kernel_devices, page_slice};

#[derive(PartialEq, Eq, Debug)]
enum find_type {
    PDE,
    PUDE,
    PTE,
}

pub const RESERVED: usize = 3;

// BOOT_CODE void map_kernel_window(void)
// {

//     paddr_t paddr;
//     pptr_t vaddr;
//     word_t idx;

//     /* place the PUD into the PGD */
//     armKSGlobalKernelPGD[GET_PGD_INDEX(PPTR_BASE)] = pgde_pgde_pud_new(
//                                                          addrFromKPPtr(armKSGlobalKernelPUD));

//     /* place all PDs except the last one in PUD */
//     for (idx = GET_PUD_INDEX(PPTR_BASE); idx < GET_PUD_INDEX(PPTR_TOP); idx++) {
//         armKSGlobalKernelPUD[idx] = pude_pude_pd_new(
//                                         addrFromKPPtr(&armKSGlobalKernelPDs[idx][0])
//                                     );
//     }

//     /* map the kernel window using large pages */
//     vaddr = PPTR_BASE;
//     for (paddr = PADDR_BASE; paddr < PADDR_TOP; paddr += BIT(seL4_LargePageBits)) {
//         armKSGlobalKernelPDs[GET_PUD_INDEX(vaddr)][GET_PD_INDEX(vaddr)] = pde_pde_large_new(
//                                                                               1, // UXN
//                                                                               paddr,
//                                                                               0,                        /* global */
//                                                                               1,                        /* access flag */
//                                                                               SMP_TERNARY(SMP_SHARE, 0),        /* Inner-shareable if SMP enabled, otherwise unshared */
//                                                                               0,                        /* VMKernelOnly */
//                                                                               NORMAL
//                                                                           );
//         vaddr += BIT(seL4_LargePageBits);
//     }

//     /* put the PD into the PUD for device window */
//     armKSGlobalKernelPUD[GET_PUD_INDEX(PPTR_TOP)] = pude_pude_pd_new(
//                                                         addrFromKPPtr(&armKSGlobalKernelPDs[BIT(PUD_INDEX_BITS) - 1][0])
//                                                     );

//     /* put the PT into the PD for device window */
//     armKSGlobalKernelPDs[BIT(PUD_INDEX_BITS) - 1][BIT(PD_INDEX_BITS) - 1] = pde_pde_small_new(
//                                                                                 addrFromKPPtr(armKSGlobalKernelPT)
//                                                                             );
// }

#[no_mangle]
#[link_section = ".boot.text"]
pub fn rust_map_kernel_window() {
    // println!("go into rusta map kernel window");
    set_kernel_page_global_directory_by_index(
        (VAddr(PPTR_BASE)).GET_KPT_INDEX(0),
        PTE::pte_new_table(kpptr_to_paddr(get_kernel_page_upper_directory_base())),
    );

    let mut idx = VAddr(PPTR_BASE).GET_KPT_INDEX(1);
    while idx < VAddr(PPTR_TOP).GET_KPT_INDEX(1) {
        set_kernel_page_upper_directory_by_index(
            idx,
            PTE::pte_new_table(kpptr_to_paddr(get_kernel_page_directory_base_by_index(idx))),
        );
        idx += 1;
    }

    let mut vaddr = PPTR_BASE;
    let mut paddr = PADDR_BASE;
    while paddr < PADDR_TOP {
        set_kernel_page_directory_by_index(
            VAddr(vaddr).GET_KPT_INDEX(1),
            VAddr(vaddr).GET_KPT_INDEX(2),
            PTE::pte_new_page(1, paddr, 0, 1, 0, 0, mair_types::NORMAL as usize),
        );

        vaddr += BIT!(seL4_LargePageBits);
        paddr += BIT!(seL4_LargePageBits)
    }

    //     /* put the PD into the PUD for device window */
    //     armKSGlobalKernelPUD[GET_PUD_INDEX(PPTR_TOP)] = pude_pude_pd_new(
    //                                                         addrFromKPPtr(&armKSGlobalKernelPDs[BIT(PUD_INDEX_BITS) - 1][0])
    //                                                     );

    //     /* put the PT into the PD for device window */
    //     armKSGlobalKernelPDs[BIT(PUD_INDEX_BITS) - 1][BIT(PD_INDEX_BITS) - 1] = pde_pde_small_new(
    //                                                                                 addrFromKPPtr(armKSGlobalKernelPT)
    //

    set_kernel_page_upper_directory_by_index(
        VAddr(PPTR_TOP).GET_KPT_INDEX(1),
        PTE::pte_new_table(kpptr_to_paddr(get_kernel_page_directory_base_by_index(
            BIT!(PUD_INDEX_BITS) - 1,
        ))),
    );
    set_kernel_page_directory_by_index(
        BIT!(PUD_INDEX_BITS) - 1,
        BIT!(PUD_INDEX_BITS) - 1,
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
    let uxn = 1;
    let attr_index: usize;
    let shareable: usize;
    if attributes.get_page_cacheable() != 0 {
        attr_index = mair_types::NORMAL as usize;
        shareable = 0;
    } else {
        attr_index = mair_types::DEVICE_nGnRnE as usize;
        shareable = 0;
    }
    set_kernel_page_table_by_index(
        VAddr(vaddr).GET_KPT_INDEX(3),
        PTE::pte_new_4k_page(
            uxn,
            paddr,
            0,
            1,
            shareable,
            PTE::ap_from_vm_rights_t(vm_rights).bits() >> 6,
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
    target_pte.set_next_level_paddr(pptr_to_paddr(pt));
    // TODO: move 0x3 into a proper position.
    target_pte.set_attr(3);
}

/// TODO: Write the comments.
#[no_mangle]
#[link_section = ".boot.text"]
pub fn map_it_pd_cap(vspace_cap: &cap_vspace_cap, pd_cap: &cap_page_table_cap) {
    let pgd = page_slice::<PTE>(vspace_cap.get_capVSBasePtr() as usize);
    let pd_addr = pd_cap.get_capPTBasePtr() as usize;
    let vptr: VAddr = (pd_cap.get_capPTMappedAddress() as usize).into();
    assert_eq!(pd_cap.get_capPTIsMapped(), 1);
    // TODO: move 0x3 into a proper position.
    assert_eq!(pgd[vptr.pgd_index()].attr(), 0x3);
    let pud = pgd[vptr.pgd_index()].next_level_slice::<PTE>();
    pud[vptr.pud_index()] = PTE::new_page(pptr_to_paddr(pd_addr), 0x3);
}

/// TODO: Write the comments.
pub fn map_it_pud_cap(vspace_cap: &cap_vspace_cap, pud_cap: &cap_page_table_cap) {
    let pgd = page_slice::<PTE>(vspace_cap.get_capVSBasePtr() as usize);
    let pud_addr = pud_cap.get_capPTBasePtr() as usize;
    let vptr: VAddr = (pud_cap.get_capPTMappedAddress() as usize).into();
    assert_eq!(pud_cap.get_capPTIsMapped(), 1);

    // TODO: move 0x3 into a proper position.
    pgd[vptr.pgd_index()] = PTE::new_page(pptr_to_paddr(pud_addr), 0x3);
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
    pte.set_attr(PTE::pte_new_4k_page((!exec) as usize, 0, 1, 1, 0, 1, 0).0);
    pte.set_next_level_paddr(pptr_to_paddr(frame_cap.get_capFBasePtr() as usize));
}

/// TODO: Write the comments.
#[link_section = ".boot.text"]
fn find_pt(vspace_root: usize, vptr: VAddr, ftype: find_type) -> usize {
    let pgd = page_slice::<PTE>(vspace_root);
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

#[no_mangle]
#[link_section = ".boot.text"]
pub fn create_it_pd_cap(vspace_cap: &cap_vspace_cap, pptr: usize, vptr: usize, asid: usize) -> cap {
    let capability = cap_page_table_cap::new(asid as u64, pptr as u64, 1, vptr as u64);
    map_it_pd_cap(vspace_cap, &capability);
    return capability.unsplay();
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn create_unmapped_it_frame_cap(pptr: pptr_t, use_large: bool) -> cap_frame_cap {
    return create_it_frame_cap(pptr, 0, 0, use_large);
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn create_it_frame_cap(
    pptr: pptr_t,
    vptr: vptr_t,
    asid: asid_t,
    use_large: bool,
) -> cap_frame_cap {
    let frame_size;
    if use_large {
        frame_size = ARM_Large_Page;
    } else {
        frame_size = ARM_Small_Page;
    }
    cap_frame_cap::new(
        asid as u64,
        pptr as u64,
        frame_size as u64,
        vptr as u64,
        vm_rights_t::VMReadWrite as u64,
        0,
    )
}

#[no_mangle]
pub fn create_mapped_it_frame_cap(
    pd_cap: &cap_vspace_cap,
    pptr: usize,
    vptr: usize,
    asid: usize,
    use_large: bool,
    exec: bool,
) -> cap_frame_cap {
    let capability = create_it_frame_cap(pptr, vptr, asid, use_large);
    map_it_frame_cap(pd_cap, &capability, exec);
    capability
}
