use super::calculate_extra_bi_size_bits;
use super::utils::{arch_get_n_paging, provide_cap, write_slot};
use super::{ndks_boot, utils::is_reg_empty};
use crate::interrupt::{set_irq_state_by_irq, IRQState};
use crate::structures::{
    create_frames_of_region_ret_t, region_t, rootserver_mem_t, v_region_t, BootInfo, SlotRegion,
};
use crate::{BIT, ROUND_DOWN};
use log::debug;
use sel4_common::arch::{ArchReg, ArchTCB};
#[cfg(feature = "kernel_mcs")]
use sel4_common::platform::{timer, Timer_func};
use sel4_common::structures::{exception_t, seL4_IPCBuffer};
#[cfg(target_arch = "riscv64")]
use sel4_common::structures_gen::cap_page_table_cap;
#[cfg(feature = "enable_smc")]
use sel4_common::structures_gen::cap_smc_cap;
#[cfg(target_arch = "aarch64")]
use sel4_common::structures_gen::cap_vspace_cap;
use sel4_common::structures_gen::{
    cap_asid_control_cap, cap_asid_pool_cap, cap_cnode_cap, cap_domain_cap, cap_frame_cap,
    cap_irq_control_cap, cap_tag, cap_thread_cap,
};
use sel4_common::utils::convert_to_mut_type_ref;
use sel4_cspace::interface::*;

use crate::utils::clear_memory;
use sel4_common::platform::{IRQ_INVALID, KERNEL_TIMER_IRQ, MAX_IRQ};
use sel4_common::sel4_config::*;

use sel4_task::*;
use sel4_vspace::*;
#[no_mangle]
#[link_section = ".boot.bss"]
pub static mut rootserver_mem: region_t = region_t { start: 0, end: 0 };

#[no_mangle]
#[link_section = ".boot.bss"]
pub static mut rootserver: rootserver_mem_t = rootserver_mem_t {
    cnode: 0,
    vspace: 0,
    asid_pool: 0,
    ipc_buf: 0,
    boot_info: 0,
    extra_bi: 0,
    tcb: 0,
    #[cfg(feature = "kernel_mcs")]
    sc: 0,
    paging: region_t {
        start: (0),
        end: (0),
    },
};

pub fn root_server_init(
    it_v_reg: v_region_t,
    extra_bi_size_bits: usize,
    ipcbuf_vptr: usize,
    bi_frame_vptr: usize,
    extra_bi_size: usize,
    extra_bi_frame_vptr: usize,
    ui_reg: region_t,
    pv_offset: isize,
    v_entry: usize,
) -> Option<(*mut tcb_t, cap_cnode_cap)> {
    unsafe {
        root_server_mem_init(it_v_reg, extra_bi_size_bits);
    }
    let root_cnode_cap = unsafe { create_root_cnode() };
    if root_cnode_cap.clone().unsplay().get_tag() == cap_tag::cap_null_cap {
        debug!("ERROR: root c-node creation failed\n");
        return None;
    }

    create_domain_cap(&root_cnode_cap);
    init_irqs(&root_cnode_cap);
    #[cfg(feature = "enable_smc")]
    init_smc(&root_cnode_cap);
    unsafe {
        rust_populate_bi_frame(0, CONFIG_MAX_NUM_NODES, ipcbuf_vptr, extra_bi_size);
    }
    let it_pd_cap = unsafe { rust_create_it_address_space(&root_cnode_cap, it_v_reg) };
    if it_pd_cap.clone().unsplay().get_tag() == cap_tag::cap_null_cap {
        debug!("ERROR: address space creation for initial thread failed");
        return None;
    }

    if !init_bi_frame_cap(
        &root_cnode_cap,
        &it_pd_cap,
        bi_frame_vptr,
        extra_bi_size,
        extra_bi_frame_vptr,
    ) {
        return None;
    }

    // #ifdef CONFIG_KERNEL_MCS
    //     init_sched_control(root_cnode_cap, CONFIG_MAX_NUM_NODES);
    // #endif
    #[cfg(feature = "kernel_mcs")]
    init_sched_control(&root_cnode_cap, CONFIG_MAX_NUM_NODES);

    let ipcbuf_cap = unsafe { create_ipcbuf_frame_cap(&root_cnode_cap, &it_pd_cap, ipcbuf_vptr) };
    if ipcbuf_cap.clone().unsplay().get_tag() == cap_tag::cap_null_cap {
        debug!("ERROR: could not create IPC buffer for initial thread");
        return None;
    }

    if ipcbuf_cap.clone().unsplay().get_tag() == cap_tag::cap_null_cap {
        debug!("ERROR: could not create IPC buffer for initial thread");
        return None;
    }
    if !create_frame_ui_frames(&root_cnode_cap, &it_pd_cap, ui_reg, pv_offset) {
        return None;
    }

    if !asid_init(&root_cnode_cap, &it_pd_cap) {
        return None;
    }
    #[cfg(feature = "kernel_mcs")]
    SET_NODE_STATE!(ksCurTime = timer.get_current_time());

    let initial = unsafe {
        create_initial_thread(
            &root_cnode_cap,
            &it_pd_cap,
            v_entry,
            bi_frame_vptr,
            ipcbuf_vptr,
            ipcbuf_cap,
        )
    };
    if initial as usize == 0 {
        debug!("ERROR: could not create initial thread");
        return None;
    }
    Some((initial, root_cnode_cap))
}

// #[no_mangle]
#[cfg(target_arch = "aarch64")]
unsafe fn create_initial_thread(
    root_cnode_cap: &cap_cnode_cap,
    it_pd_cap: &cap_vspace_cap,
    ui_v_entry: usize,
    bi_frame_vptr: usize,
    ipcbuf_vptr: usize,
    ipcbuf_cap: cap_frame_cap,
) -> *mut tcb_t {
    #[cfg(feature = "kernel_mcs")]
    use sel4_common::{
        arch::us_to_ticks, platform::time_def::US_IN_MS, structures_gen::cap_sched_context_cap,
    };
    let tcb = convert_to_mut_type_ref::<tcb_t>(rootserver.tcb + TCB_OFFSET);
    #[cfg(feature = "kernel_mcs")]
    {
        tcb.tcbTimeSlice = CONFIG_TIME_SLICE;
    }
    tcb.tcbArch = ArchTCB::default();
    let cnode = convert_to_mut_type_ref::<cte_t>(root_cnode_cap.get_capCNodePtr() as usize);
    let ipc_buf_slot = cnode.get_offset_slot(SEL4_CAP_INIT_THREAD_IPC_BUFFER);
    let dc_ret = ipc_buf_slot.derive_cap(&ipcbuf_cap.unsplay().clone());
    if dc_ret.status != exception_t::EXCEPTION_NONE {
        debug!("Failed to derive copy of IPC Buffer\n");
        return 0 as *mut tcb_t;
    }

    cte_insert(
        &root_cnode_cap.clone().unsplay(),
        cnode.get_offset_slot(SEL4_CAP_INIT_THREAD_CNODE),
        tcb.get_cspace_mut_ref(TCB_CTABLE),
    );

    cte_insert(
        &it_pd_cap.clone().unsplay(),
        cnode.get_offset_slot(SEL4_CAP_INIT_THREAD_VSPACE),
        tcb.get_cspace_mut_ref(TCB_VTABLE),
    );

    cte_insert(
        &dc_ret.capability,
        cnode.get_offset_slot(SEL4_CAP_INIT_THREAD_IPC_BUFFER),
        tcb.get_cspace_mut_ref(TCB_BUFFER),
    );

    tcb.tcbIPCBuffer = ipcbuf_vptr;
    tcb.tcbArch.set_register(ArchReg::Cap, bi_frame_vptr);
    tcb.tcbArch.set_register(ArchReg::NEXT_IP, ui_v_entry);
    #[cfg(feature = "kernel_mcs")]
    {
        configure_sched_context(
            tcb,
            convert_to_mut_type_ref(rootserver.sc),
            us_to_ticks(CONFIG_BOOT_THREAD_TIME_SLICE * US_IN_MS),
        );
    }
    tcb.tcbMCP = SEL4_MAX_PRIO;
    tcb.tcbPriority = SEL4_MAX_PRIO;
    set_thread_state(tcb, ThreadState::ThreadStateRunning);
    #[cfg(not(feature = "kernel_mcs"))]
    tcb.setup_reply_master();
    ksCurDomain = ksDomSchedule[ksDomScheduleIdx].domain;
    #[cfg(not(feature = "kernel_mcs"))]
    {
        ksDomainTime = ksDomSchedule[ksDomScheduleIdx].length;
    }
    #[cfg(feature = "kernel_mcs")]
    {
        ksDomainTime = us_to_ticks(ksDomSchedule[ksDomScheduleIdx].length * US_IN_MS);
    }
    tcb.domain = ksCurDomain;
    // log::error!("tcb.domain:{:#x}", &tcb.domain as *const usize as usize);
    #[cfg(all(eature = "enable_smp", not(feature = "kernel_mcs")))]
    {
        tcb.tcbAffinity = 0;
    }

    let capability = cap_thread_cap::new(tcb.get_ptr() as u64).unsplay();
    write_slot(
        cnode.get_offset_slot(SEL4_CAP_INIT_THREAD_TCB) as *mut cte_t,
        capability,
    );
    #[cfg(feature = "kernel_mcs")]
    {
        let capability = cap_sched_context_cap::new(
            tcb.tcbSchedContext as u64,
            SEL4_MIN_SCHED_CONTEXT_BITS as u64,
        )
        .unsplay();
        write_slot(
            cnode.get_offset_slot(SEL4_CAP_INIT_THREAD_SC) as *mut cte_t,
            capability,
        );
    }
    // forget(*tcb);
    tcb as *mut tcb_t
}
// #[no_mangle]
#[cfg(target_arch = "riscv64")]
unsafe fn create_initial_thread(
    root_cnode_cap: &cap_cnode_cap,
    it_pd_cap: &cap_page_table_cap,
    ui_v_entry: usize,
    bi_frame_vptr: usize,
    ipcbuf_vptr: usize,
    ipcbuf_cap: cap_frame_cap,
) -> *mut tcb_t {
    #[cfg(feature = "kernel_mcs")]
    use sel4_common::{
        arch::us_to_ticks, platform::time_def::US_IN_MS, structures_gen::cap_sched_context_cap,
    };
    let tcb = convert_to_mut_type_ref::<tcb_t>(rootserver.tcb + TCB_OFFSET);
    #[cfg(feature = "kernel_mcs")]
    {
        tcb.tcbTimeSlice = CONFIG_TIME_SLICE;
    }
    tcb.tcbArch = ArchTCB::default();

    let cnode = convert_to_mut_type_ref::<cte_t>(root_cnode_cap.get_capCNodePtr() as usize);
    let ipc_buf_slot = cnode.get_offset_slot(SEL4_CAP_INIT_THREAD_IPC_BUFFER);
    let dc_ret = ipc_buf_slot.derive_cap(&ipcbuf_cap.unsplay().clone());
    if dc_ret.status != exception_t::EXCEPTION_NONE {
        debug!("Failed to derive copy of IPC Buffer\n");
        return 0 as *mut tcb_t;
    }

    cte_insert(
        &root_cnode_cap.clone().unsplay(),
        cnode.get_offset_slot(SEL4_CAP_INIT_THREAD_CNODE),
        tcb.get_cspace_mut_ref(TCB_CTABLE),
    );

    cte_insert(
        &it_pd_cap.clone().unsplay(),
        cnode.get_offset_slot(SEL4_CAP_INIT_THREAD_VSPACE),
        tcb.get_cspace_mut_ref(TCB_VTABLE),
    );

    cte_insert(
        &dc_ret.capability,
        cnode.get_offset_slot(SEL4_CAP_INIT_THREAD_IPC_BUFFER),
        tcb.get_cspace_mut_ref(TCB_BUFFER),
    );

    tcb.tcbIPCBuffer = ipcbuf_vptr;
    tcb.tcbArch.set_register(ArchReg::Cap, bi_frame_vptr);
    tcb.tcbArch.set_register(ArchReg::NEXT_IP, ui_v_entry);
    #[cfg(feature = "kernel_mcs")]
    {
        configure_sched_context(
            tcb,
            convert_to_mut_type_ref(rootserver.sc),
            us_to_ticks(CONFIG_BOOT_THREAD_TIME_SLICE * US_IN_MS),
        );
    }
    tcb.tcbMCP = SEL4_MAX_PRIO;
    tcb.tcbPriority = SEL4_MAX_PRIO;
    set_thread_state(tcb, ThreadState::ThreadStateRunning);
    #[cfg(not(feature = "kernel_mcs"))]
    tcb.setup_reply_master();
    ksCurDomain = ksDomSchedule[ksDomScheduleIdx].domain;
    #[cfg(not(feature = "kernel_mcs"))]
    {
        ksDomainTime = ksDomSchedule[ksDomScheduleIdx].length;
    }
    #[cfg(feature = "kernel_mcs")]
    {
        ksDomainTime = us_to_ticks(ksDomSchedule[ksDomScheduleIdx].length * US_IN_MS);
    }
    ksDomainTime = ksDomSchedule[ksDomScheduleIdx].length;
    tcb.domain = ksCurDomain;
    // log::error!("tcb.domain:{:#x}", &tcb.domain as *const usize as usize);
    #[cfg(all(eature = "enable_smp", not(feature = "kernel_mcs")))]
    {
        tcb.tcbAffinity = 0;
    }

    let capability = cap_thread_cap::new(tcb.get_ptr() as u64).unsplay();
    write_slot(
        cnode.get_offset_slot(SEL4_CAP_INIT_THREAD_TCB) as *mut cte_t,
        capability,
    );
    #[cfg(feature = "kernel_mcs")]
    {
        let capability = cap_sched_context_cap::new(
            tcb.tcbSchedContext as u64,
            SEL4_MIN_SCHED_CONTEXT_BITS as u64,
        )
        .unsplay();
        write_slot(
            cnode.get_offset_slot(SEL4_CAP_INIT_THREAD_SC) as *mut cte_t,
            capability,
        );
    }
    // forget(*tcb);
    tcb as *mut tcb_t
}

#[cfg(target_arch = "aarch64")]
fn asid_init(root_cnode_cap: &cap_cnode_cap, it_pd_cap: &cap_vspace_cap) -> bool {
    let it_ap_cap = create_it_asid_pool(root_cnode_cap);
    if it_ap_cap.clone().unsplay().get_tag() == cap_tag::cap_null_cap {
        debug!("ERROR: could not create ASID pool for initial thread");
        return false;
    }
    write_it_asid_pool(&it_ap_cap, it_pd_cap);
    true
}
#[cfg(target_arch = "riscv64")]
fn asid_init(root_cnode_cap: &cap_cnode_cap, it_pd_cap: &cap_page_table_cap) -> bool {
    let it_ap_cap = create_it_asid_pool(root_cnode_cap);
    if it_ap_cap.clone().unsplay().get_tag() == cap_tag::cap_null_cap {
        debug!("ERROR: could not create ASID pool for initial thread");
        return false;
    }
    unsafe {
        let ap = it_ap_cap.get_capASIDPool() as usize;
        let ptr = (ap + 8 * IT_ASID) as *mut usize;
        *ptr = it_pd_cap.get_capPTBasePtr() as usize;
        riscvKSASIDTable[IT_ASID >> ASID_LOW_BITS] = ap as *mut asid_pool_t;
    }
    true
}

fn create_it_asid_pool(root_cnode_cap: &cap_cnode_cap) -> cap_asid_pool_cap {
    log::debug!("root_server.asid_pool: {:#x}", unsafe {
        rootserver.asid_pool
    });
    let ap_cap = unsafe {
        cap_asid_pool_cap::new(
            (IT_ASID >> ASID_LOW_BITS) as u64,
            rootserver.asid_pool as u64,
        )
    };
    unsafe {
        let ptr = root_cnode_cap.get_capCNodePtr() as *mut cte_t;
        write_slot(
            ptr.add(SEL4_CAP_INIT_THREAD_ASID_POOL),
            ap_cap.clone().unsplay(),
        );
        write_slot(
            ptr.add(SEL4_CAP_ASID_CONTROL),
            cap_asid_control_cap::new().unsplay(),
        );
    }
    log::debug!(
        "asid_init needed to create: {:p} {:#x}",
        &ap_cap.clone(),
        ap_cap.get_capASIDPool()
    );
    ap_cap
}
#[cfg(feature = "enable_smc")]
pub fn init_smc(root_cnode_cap: &cap_cnode_cap) {
    let capability = cap_smc_cap::new(0).unsplay();
    unsafe {
        let pos = root_cnode_cap.get_capCNodePtr() as *mut cte_t;
        write_slot(pos.add(SEL4_CAP_SMC), capability);
    }
}

#[cfg(feature = "kernel_mcs")]
//TODO: MCS: Done
fn init_sched_control(root_cnode_cap: &cap_cnode_cap, num_nodes: usize) -> bool {
    use sel4_common::structures_gen::cap_sched_control_cap;

    let slot_pos_before = unsafe { ndks_boot.slot_pos_cur };

    /* create a sched control cap for each core */
    for i in 0..num_nodes {
        if !provide_cap(
            root_cnode_cap,
            cap_sched_control_cap::new(i as u64).unsplay(),
        ) {
            log::debug!(
                "can't init sched_control for node {}, provide_cap() failed\n",
                i
            );
            return false;
        }
    }

    /* update boot info with slot region for sched control caps */
    unsafe {
        (*ndks_boot.bi_frame).schedcontrol = SlotRegion {
            start: slot_pos_before,
            end: ndks_boot.slot_pos_cur,
        }
    };

    true
}

#[cfg(target_arch = "aarch64")]
fn create_frame_ui_frames(
    root_cnode_cap: &cap_cnode_cap,
    it_pd_cap: &cap_vspace_cap,
    ui_reg: region_t,
    pv_offset: isize,
) -> bool {
    let create_frames_ret = rust_create_frames_of_region(
        &root_cnode_cap,
        &it_pd_cap,
        ui_reg,
        true,
        pv_offset as isize,
    );
    if !create_frames_ret.success {
        debug!("ERROR: could not create all userland image frames");
        return false;
    }
    unsafe {
        (*ndks_boot.bi_frame).userImageFrames = create_frames_ret.region;
    }
    true
}
#[cfg(target_arch = "riscv64")]
fn create_frame_ui_frames(
    root_cnode_cap: &cap_cnode_cap,
    it_pd_cap: &cap_page_table_cap,
    ui_reg: region_t,
    pv_offset: isize,
) -> bool {
    let create_frames_ret = rust_create_frames_of_region(
        &root_cnode_cap,
        &it_pd_cap,
        ui_reg,
        true,
        pv_offset as isize,
    );
    if !create_frames_ret.success {
        debug!("ERROR: could not create all userland image frames");
        return false;
    }
    unsafe {
        (*ndks_boot.bi_frame).userImageFrames = create_frames_ret.region;
    }
    true
}

unsafe fn root_server_mem_init(it_v_reg: v_region_t, extra_bi_size_bits: usize) {
    let size = calculate_rootserver_size(it_v_reg, extra_bi_size_bits);
    let max = rootserver_max_size_bits(extra_bi_size_bits);
    let mut i = ndks_boot.freemem.len() - 1;
    /* skip any empty regions */
    while i != usize::MAX && is_reg_empty(&ndks_boot.freemem[i]) {
        i -= 1;
    }
    while i != usize::MAX && i < ndks_boot.freemem.len() {
        /* Invariant: both i and (i + 1) are valid indices in ndks_boot.freemem. */
        assert!(i < (ndks_boot.freemem.len() - 1));
        /* Invariant; the region at index i is the current candidate.
         * Invariant: regions 0 up to (i - 1), if any, are additional candidates.
         * Invariant: region (i + 1) is empty. */
        assert!(is_reg_empty(&ndks_boot.freemem[i + 1]));

        let empty_index = i + 1;
        let unaligned_start = ndks_boot.freemem[i].end - size;
        let start = ROUND_DOWN!(unaligned_start, max);

        /* if unaligned_start didn't underflow, and start fits in the region,
         * then we've found a region that fits the root server objects. */
        if unaligned_start <= ndks_boot.freemem[i].end && start >= ndks_boot.freemem[i].start {
            create_rootserver_objects(start, it_v_reg, extra_bi_size_bits);
            ndks_boot.freemem[empty_index] = region_t {
                start: start + size,
                end: ndks_boot.freemem[i].end,
            };
            ndks_boot.freemem[i].end = start;
            return;
        }
        /* Region i isn't big enough, so shuffle it up to slot (i + 1),
         * which we know is unused. */
        ndks_boot.freemem[empty_index] = ndks_boot.freemem[i];
        ndks_boot.freemem[i] = region_t { start: 0, end: 0 };
        i -= 1;
    }
}

unsafe fn create_root_cnode() -> cap_cnode_cap {
    let capability = cap_cnode_cap::new(
        0,
        (WORD_BITS - CONFIG_ROOT_CNODE_SIZE_BITS) as u64,
        CONFIG_ROOT_CNODE_SIZE_BITS as u64,
        rootserver.cnode as u64,
    );
    let ptr = rootserver.cnode as *mut cte_t;
    write_slot(
        ptr.add(SEL4_CAP_INIT_THREAD_CNODE),
        capability.clone().unsplay(),
    );
    capability
}

fn calculate_rootserver_size(it_v_reg: v_region_t, extra_bi_size_bits: usize) -> usize {
    let mut size = BIT!(CONFIG_ROOT_CNODE_SIZE_BITS + SEL4_SLOT_BITS);
    size += BIT!(SEL4_TCB_BITS);
    size += BIT!(SEL4_PAGE_BITS);
    size += BIT!(BI_FRAME_SIZE_BITS);
    size += BIT!(SEL4_ASID_POOL_BITS);
    size += if extra_bi_size_bits > 0 {
        BIT!(extra_bi_size_bits)
    } else {
        0
    };
    size += BIT!(SEL4_VSPACE_BITS);
    #[cfg(feature = "kernel_mcs")]
    {
        size += BIT!(SEL4_MIN_SCHED_CONTEXT_BITS);
    }
    return size + arch_get_n_paging(it_v_reg) * BIT!(SEL4_PAGE_TABLE_BITS);
}

fn rootserver_max_size_bits(extra_bi_size_bits: usize) -> usize {
    let cnode_size_bits = CONFIG_ROOT_CNODE_SIZE_BITS + SEL4_SLOT_BITS;
    let maxx = if cnode_size_bits > SEL4_VSPACE_BITS {
        cnode_size_bits
    } else {
        SEL4_VSPACE_BITS
    };
    if maxx > extra_bi_size_bits {
        maxx
    } else {
        extra_bi_size_bits
    }
}

fn alloc_rootserver_obj(size_bits: usize, n: usize) -> usize {
    unsafe {
        let allocated = rootserver_mem.start;
        assert!(allocated % BIT!(size_bits) == 0);
        rootserver_mem.start += n * BIT!(size_bits);
        assert!(rootserver_mem.start <= rootserver_mem.end);
        allocated
    }
}

#[inline]
unsafe fn it_alloc_paging() -> usize {
    let allocated = rootserver.paging.start;
    rootserver.paging.start += BIT!(SEL4_PAGE_TABLE_BITS);
    assert!(rootserver.paging.start <= rootserver.paging.end);
    allocated
}

unsafe fn maybe_alloc_extra_bi(cmp_size_bits: usize, extra_bi_size_bits: usize) {
    if extra_bi_size_bits >= cmp_size_bits && rootserver.extra_bi == 0 {
        rootserver.extra_bi = alloc_rootserver_obj(extra_bi_size_bits, 1);
    }
}

unsafe fn create_rootserver_objects(start: usize, it_v_reg: v_region_t, extra_bi_size_bits: usize) {
    let cnode_size_bits = CONFIG_ROOT_CNODE_SIZE_BITS + SEL4_SLOT_BITS;
    let max = rootserver_max_size_bits(extra_bi_size_bits);

    let size = calculate_rootserver_size(it_v_reg, extra_bi_size_bits);
    rootserver_mem.start = start;
    rootserver_mem.end = start + size;
    maybe_alloc_extra_bi(max, extra_bi_size_bits);

    rootserver.cnode = alloc_rootserver_obj(cnode_size_bits, 1);
    maybe_alloc_extra_bi(SEL4_VSPACE_BITS, extra_bi_size_bits);
    rootserver.vspace = alloc_rootserver_obj(SEL4_VSPACE_BITS, 1);

    maybe_alloc_extra_bi(SEL4_PAGE_BITS, extra_bi_size_bits);
    rootserver.asid_pool = alloc_rootserver_obj(SEL4_ASID_POOL_BITS, 1);
    rootserver.ipc_buf = alloc_rootserver_obj(SEL4_PAGE_BITS, 1);
    rootserver.boot_info = alloc_rootserver_obj(BI_FRAME_SIZE_BITS, 1);

    let n = arch_get_n_paging(it_v_reg);
    rootserver.paging.start = alloc_rootserver_obj(SEL4_PAGE_TABLE_BITS, n);
    rootserver.paging.end = rootserver.paging.start + n * BIT!(SEL4_PAGE_TABLE_BITS);
    rootserver.tcb = alloc_rootserver_obj(SEL4_TCB_BITS, 1);

    #[cfg(feature = "kernel_mcs")]
    {
        rootserver.sc = alloc_rootserver_obj(SEL4_MIN_SCHED_CONTEXT_BITS, 1);
    }

    assert_eq!(rootserver_mem.start, rootserver_mem.end);
}

fn create_domain_cap(root_cnode_cap: &cap_cnode_cap) {
    assert!(KS_DOM_SCHEDULE_LENGTH > 0);
    for i in 0..KS_DOM_SCHEDULE_LENGTH {
        unsafe {
            assert!(ksDomSchedule[i].domain < CONFIG_NUM_DOMAINS);
            assert!(ksDomSchedule[i].length > 0);
        }
    }
    let capability = cap_domain_cap::new().unsplay();
    unsafe {
        let pos = root_cnode_cap.get_capCNodePtr() as *mut cte_t;
        write_slot(pos.add(SEL4_CAP_DOMAIN), capability);
    }
}

fn init_irqs(root_cnode_cap: &cap_cnode_cap) {
    for i in 0..MAX_IRQ + 1 {
        if i != IRQ_INVALID {
            set_irq_state_by_irq(IRQState::IRQInactive, i);
        }
    }
    set_irq_state_by_irq(IRQState::IRQTimer, KERNEL_TIMER_IRQ);
    #[cfg(all(feature = "enable_smp", target_arch = "riscv64"))]
    {
        use sel4_common::platform::{INTERRUPT_IPI_0, INTERRUPT_IPI_1};
        set_irq_state_by_irq(IRQState::IRQIPI, INTERRUPT_IPI_0);
        set_irq_state_by_irq(IRQState::IRQIPI, INTERRUPT_IPI_1);
    }
    #[cfg(all(feature = "enable_smp", target_arch = "aarch64"))]
    {
        use sel4_common::arch::config::{IRQ_REMOTE_CALL_IPI, IRQ_RESCHEDULE_IPI};
        set_irq_state_by_irq(IRQState::IRQIPI, IRQ_REMOTE_CALL_IPI);
        set_irq_state_by_irq(IRQState::IRQIPI, IRQ_RESCHEDULE_IPI);
    }

    unsafe {
        let ptr = root_cnode_cap.get_capCNodePtr() as *mut cte_t;
        write_slot(
            ptr.add(SEL4_CAP_IRQ_CONTROL),
            cap_irq_control_cap::new().unsplay(),
        );
    }
}

#[cfg(target_arch = "riscv64")]
unsafe fn rust_create_it_address_space(
    root_cnode_cap: &cap_cnode_cap,
    it_v_reg: v_region_t,
) -> cap_page_table_cap {
    copyGlobalMappings(rootserver.vspace);
    let lvl1pt_cap = cap_page_table_cap::new(
        IT_ASID as u64,
        rootserver.vspace as u64,
        1,
        rootserver.vspace as u64,
    );
    let ptr = root_cnode_cap.get_capCNodePtr() as *mut cte_t;
    let slot_pos_before = ndks_boot.slot_pos_cur;
    write_slot(
        ptr.add(SEL4_CAP_INIT_THREAD_VSPACE),
        lvl1pt_cap.clone().unsplay(),
    );
    let mut i = 0;
    while i < CONFIG_PT_LEVELS - 1 {
        let mut pt_vptr = ROUND_DOWN!(it_v_reg.start, riscv_get_lvl_pgsize_bits(i));
        while pt_vptr < it_v_reg.end {
            if !provide_cap(
                root_cnode_cap,
                create_it_pt_cap(&lvl1pt_cap, it_alloc_paging(), pt_vptr, IT_ASID).unsplay(),
            ) {
                return cap_page_table_cap::new(0, 0, 0, 0);
            }
            pt_vptr += riscv_get_lvl_pgsize(i);
        }
        i += 1;
    }
    let slot_pos_after = ndks_boot.slot_pos_cur;
    (*ndks_boot.bi_frame).userImagePaging = SlotRegion {
        start: slot_pos_before,
        end: slot_pos_after,
    };
    lvl1pt_cap
}

#[cfg(target_arch = "aarch64")]
unsafe fn rust_create_it_address_space(
    root_cnode_cap: &cap_cnode_cap,
    it_v_reg: v_region_t,
) -> cap_vspace_cap {
    // create the PGD

    let vspace_cap = cap_vspace_cap::new(IT_ASID as u64, rootserver.vspace as u64, 1);
    let ptr = root_cnode_cap.get_capCNodePtr() as *mut cte_t;
    let slot_pos_before = ndks_boot.slot_pos_cur;
    write_slot(
        ptr.add(SEL4_CAP_INIT_THREAD_VSPACE),
        vspace_cap.clone().unsplay(),
    );

    // lxy: use these constants defined in sel4_config
    // // Create any PUDs needed for the user land image, should config `PGD_INDEX_OFFSET`, `PUD_INDEX_OFFSET`...
    // let PGD_INDEX_OFFSET = PAGE_BITS + PT_INDEX_BITS * 3;
    // let PUD_INDEX_OFFSET = PAGE_BITS + PT_INDEX_BITS * 2;
    // let PD_INDEX_OFFSET = PAGE_BITS + PT_INDEX_BITS;
    let mut vptr = ROUND_DOWN!(it_v_reg.start, PGD_INDEX_OFFSET);
    while vptr < it_v_reg.end {
        if !provide_cap(
            root_cnode_cap,
            create_it_pud_cap(&vspace_cap, it_alloc_paging(), vptr, IT_ASID).unsplay(),
        ) {
            return cap_vspace_cap::new(0, 0, 0);
        }
        vptr += BIT!(PGD_INDEX_OFFSET);
    }

    // Create any PDs needed for the user land image
    vptr = ROUND_DOWN!(it_v_reg.start, PUD_INDEX_OFFSET);
    while vptr < it_v_reg.end {
        if !provide_cap(
            root_cnode_cap,
            create_it_pd_cap(&vspace_cap, it_alloc_paging(), vptr, IT_ASID),
        ) {
            return cap_vspace_cap::new(0, 0, 0);
        }
        vptr += BIT!(PUD_INDEX_OFFSET);
    }

    // Create any PTs needed for the user land image
    vptr = ROUND_DOWN!(it_v_reg.start, PD_INDEX_OFFSET);
    while vptr < it_v_reg.end {
        if !provide_cap(
            root_cnode_cap,
            create_it_pt_cap(&vspace_cap, it_alloc_paging(), vptr, IT_ASID).unsplay(),
        ) {
            return cap_vspace_cap::new(0, 0, 0);
        }
        vptr += BIT!(PD_INDEX_OFFSET);
    }

    let slot_pos_after = ndks_boot.slot_pos_cur;
    (*ndks_boot.bi_frame).userImagePaging = SlotRegion {
        start: slot_pos_before,
        end: slot_pos_after,
    };
    vspace_cap
}

#[cfg(target_arch = "aarch64")]
fn init_bi_frame_cap(
    root_cnode_cap: &cap_cnode_cap,
    it_pd_cap: &cap_vspace_cap,
    bi_frame_vptr: usize,
    extra_bi_size: usize,
    extra_bi_frame_vptr: usize,
) -> bool {
    unsafe {
        create_bi_frame_cap(root_cnode_cap, it_pd_cap, bi_frame_vptr);
    }
    if extra_bi_size > 0 {
        let extra_bi_region = unsafe {
            region_t {
                start: rootserver.extra_bi,
                end: rootserver.extra_bi + extra_bi_size,
            }
        };
        let extra_bi_ret = rust_create_frames_of_region(
            root_cnode_cap,
            it_pd_cap,
            extra_bi_region,
            true,
            pptr_to_paddr(extra_bi_region.start) as isize - extra_bi_frame_vptr as isize,
        );

        if !extra_bi_ret.success {
            debug!("ERROR: mapping extra boot info to initial thread failed");
            return false;
        }
        unsafe {
            (*ndks_boot.bi_frame).extraBIPages = extra_bi_ret.region;
        }
    }
    true
}
#[cfg(target_arch = "riscv64")]
fn init_bi_frame_cap(
    root_cnode_cap: &cap_cnode_cap,
    it_pd_cap: &cap_page_table_cap,
    bi_frame_vptr: usize,
    extra_bi_size: usize,
    extra_bi_frame_vptr: usize,
) -> bool {
    unsafe {
        create_bi_frame_cap(root_cnode_cap, it_pd_cap, bi_frame_vptr);
    }
    if extra_bi_size > 0 {
        let extra_bi_region = unsafe {
            region_t {
                start: rootserver.extra_bi,
                end: rootserver.extra_bi + extra_bi_size,
            }
        };
        let extra_bi_ret = rust_create_frames_of_region(
            root_cnode_cap,
            it_pd_cap,
            extra_bi_region,
            true,
            pptr_to_paddr(extra_bi_region.start) as isize - extra_bi_frame_vptr as isize,
        );

        if !extra_bi_ret.success {
            debug!("ERROR: mapping extra boot info to initial thread failed");
            return false;
        }
        unsafe {
            (*ndks_boot.bi_frame).extraBIPages = extra_bi_ret.region;
        }
    }
    true
}

#[cfg(target_arch = "aarch64")]
fn rust_create_frames_of_region(
    root_cnode_cap: &cap_cnode_cap,
    pd_cap: &cap_vspace_cap,
    reg: region_t,
    do_map: bool,
    pv_offset: isize,
) -> create_frames_of_region_ret_t {
    let slot_pos_before = unsafe { ndks_boot.slot_pos_cur };
    let mut f = reg.start;
    let mut frame_cap: cap_frame_cap;
    while f < reg.end {
        if do_map {
            frame_cap = create_mapped_it_frame_cap(
                pd_cap,
                f,
                pptr_to_paddr((f as isize - pv_offset) as usize),
                IT_ASID,
                false,
                true,
            );
        } else {
            frame_cap = create_unmapped_it_frame_cap(f, false);
        }

        if !provide_cap(root_cnode_cap, frame_cap.unsplay()) {
            return create_frames_of_region_ret_t {
                region: SlotRegion { start: 0, end: 0 },
                success: false,
            };
        }
        f += BIT!(PAGE_BITS);
    }
    unsafe {
        let slot_pos_after = ndks_boot.slot_pos_cur;
        return create_frames_of_region_ret_t {
            region: SlotRegion {
                start: slot_pos_before,
                end: slot_pos_after,
            },
            success: true,
        };
    }
}
#[cfg(target_arch = "riscv64")]
fn rust_create_frames_of_region(
    root_cnode_cap: &cap_cnode_cap,
    pd_cap: &cap_page_table_cap,
    reg: region_t,
    do_map: bool,
    pv_offset: isize,
) -> create_frames_of_region_ret_t {
    let slot_pos_before = unsafe { ndks_boot.slot_pos_cur };
    let mut f = reg.start;
    let mut frame_cap: cap_frame_cap;
    while f < reg.end {
        if do_map {
            frame_cap = create_mapped_it_frame_cap(
                pd_cap,
                f,
                pptr_to_paddr((f as isize - pv_offset) as usize),
                IT_ASID,
                false,
                true,
            );
        } else {
            frame_cap = create_unmapped_it_frame_cap(f, false);
        }

        if !provide_cap(root_cnode_cap, frame_cap.unsplay()) {
            return create_frames_of_region_ret_t {
                region: SlotRegion { start: 0, end: 0 },
                success: false,
            };
        }
        f += BIT!(PAGE_BITS);
    }
    unsafe {
        let slot_pos_after = ndks_boot.slot_pos_cur;
        return create_frames_of_region_ret_t {
            region: SlotRegion {
                start: slot_pos_before,
                end: slot_pos_after,
            },
            success: true,
        };
    }
}

#[cfg(target_arch = "aarch64")]
unsafe fn create_bi_frame_cap(
    root_cnode_cap: &cap_cnode_cap,
    pd_cap: &cap_vspace_cap,
    vptr: usize,
) {
    let capability =
        create_mapped_it_frame_cap(pd_cap, rootserver.boot_info, vptr, IT_ASID, false, false);
    let ptr = root_cnode_cap.get_capCNodePtr() as *mut cte_t;
    write_slot(ptr.add(SEL4_CAP_BOOT_INFO_FRAME), capability.unsplay());
}
#[cfg(target_arch = "riscv64")]
unsafe fn create_bi_frame_cap(
    root_cnode_cap: &cap_cnode_cap,
    pd_cap: &cap_page_table_cap,
    vptr: usize,
) {
    let capability =
        create_mapped_it_frame_cap(pd_cap, rootserver.boot_info, vptr, IT_ASID, false, false);
    let ptr = root_cnode_cap.get_capCNodePtr() as *mut cte_t;
    write_slot(ptr.add(SEL4_CAP_BOOT_INFO_FRAME), capability.unsplay());
}

unsafe fn rust_populate_bi_frame(
    node_id: usize,
    num_nodes: usize,
    ipcbuf_vptr: usize,
    extra_bi_size: usize,
) {
    clear_memory(rootserver.boot_info as *mut u8, BI_FRAME_SIZE_BITS);
    if extra_bi_size != 0 {
        clear_memory(
            rootserver.extra_bi as *mut u8,
            calculate_extra_bi_size_bits(extra_bi_size),
        );
    }
    let bi = &mut *(rootserver.boot_info as *mut BootInfo);
    bi.nodeID = node_id;
    bi.numNodes = num_nodes;
    bi.numIOPTLevels = 0;
    bi.ipcBuffer = ipcbuf_vptr as *mut seL4_IPCBuffer;
    bi.initThreadCNodeSizeBits = CONFIG_ROOT_CNODE_SIZE_BITS;
    bi.initThreadDomain = ksDomSchedule[ksDomScheduleIdx].domain;
    bi.extraLen = extra_bi_size;

    ndks_boot.bi_frame = bi as *mut BootInfo;
    ndks_boot.slot_pos_cur = SEL4_NUM_INITIAL_CAPS;
}
#[cfg(target_arch = "aarch64")]
unsafe fn create_ipcbuf_frame_cap(
    root_cnode_cap: &cap_cnode_cap,
    pd_cap: &cap_vspace_cap,
    vptr: usize,
) -> cap_frame_cap {
    clear_memory(rootserver.ipc_buf as *mut u8, PAGE_BITS);
    let capability =
        create_mapped_it_frame_cap(pd_cap, rootserver.ipc_buf, vptr, IT_ASID, false, false);
    let ptr = root_cnode_cap.get_capCNodePtr() as *mut cte_t;
    write_slot(
        ptr.add(SEL4_CAP_INIT_THREAD_IPC_BUFFER),
        capability.clone().unsplay(),
    );
    return capability;
}
#[cfg(target_arch = "riscv64")]
unsafe fn create_ipcbuf_frame_cap(
    root_cnode_cap: &cap_cnode_cap,
    pd_cap: &cap_page_table_cap,
    vptr: usize,
) -> cap_frame_cap {
    clear_memory(rootserver.ipc_buf as *mut u8, PAGE_BITS);
    let capability =
        create_mapped_it_frame_cap(pd_cap, rootserver.ipc_buf, vptr, IT_ASID, false, false);
    let ptr = root_cnode_cap.get_capCNodePtr() as *mut cte_t;
    write_slot(
        ptr.add(SEL4_CAP_INIT_THREAD_IPC_BUFFER),
        capability.clone().unsplay(),
    );
    return capability;
}
