//! `CSpace Table Entry`相关操作的具体实现，包含`cte`链表的插入删除等。
use super::{
    capability::{is_cap_revocable, same_object_as, same_region_as},
    deps::{finaliseCap, post_cap_deletion, preemptionPoint},
    structures::{finaliseSlot_ret, resolveAddressBits_ret_t},
};
use crate::capability::{
    cap_func,
    zombie::{capCyclicZombie, zombie_func},
};
use core::intrinsics::{likely, unlikely};
use core::ptr;
use sel4_common::utils::{convert_to_option_mut_type_ref, MAX_FREE_INDEX};
use sel4_common::{
    sel4_bitfield_types::Bitfield,
    structures_gen::{cap, cap_cnode_cap, cap_null_cap, cap_tag, mdb_node},
};
use sel4_common::{
    sel4_config::wordRadix,
    structures::exception_t,
    utils::{convert_to_mut_type_ref, convert_to_type_ref},
    MASK,
};

#[repr(C)]
#[derive(Clone)]
pub struct deriveCap_ret {
    pub status: exception_t,
    pub capability: cap,
}

/// 由cap_t和 mdb_node 组成，是CSpace的基本组成单元
#[repr(C)]
#[derive(Clone, Debug)]
pub struct cte_t {
    pub capability: cap,
    pub cteMDBNode: mdb_node,
}

impl cte_t {
    pub fn get_ptr(&self) -> usize {
        self as *const cte_t as usize
    }

    pub fn get_offset_slot(&self, index: usize) -> &'static mut Self {
        convert_to_mut_type_ref::<Self>(self.get_ptr() + core::mem::size_of::<cte_t>() * index)
    }

    pub fn derive_cap(&self, capability: &cap) -> deriveCap_ret {
        if capability.isArchCap() {
            return self.arch_derive_cap(capability);
        }
        let mut ret = deriveCap_ret {
            status: exception_t::EXCEPTION_NONE,
            capability: cap_null_cap::new().unsplay(),
        };

        match capability.get_tag() {
            cap_tag::cap_zombie_cap => {
                ret.capability = cap_null_cap::new().unsplay();
            }
            cap_tag::cap_untyped_cap => {
                ret.status = self.ensure_no_children();
                if ret.status != exception_t::EXCEPTION_NONE {
                    ret.capability = cap_null_cap::new().unsplay();
                } else {
                    ret.capability = capability.clone();
                }
            }
            cap_tag::cap_reply_cap => {
                ret.capability = cap_null_cap::new().unsplay();
            }
            cap_tag::cap_irq_control_cap => {
                ret.capability = cap_null_cap::new().unsplay();
            }
            _ => {
                ret.capability = capability.clone();
            }
        }
        ret
    }
    /// 判断当前`cte`是否存在派生出来的子节点
    pub fn ensure_no_children(&self) -> exception_t {
        if self.cteMDBNode.get_mdbNext() != 0 {
            let next = convert_to_type_ref::<cte_t>(self.cteMDBNode.get_mdbNext() as usize);
            if self.is_mdb_parent_of(next) {
                return exception_t::EXCEPTION_SYSCALL_ERROR;
            }
        }
        exception_t::EXCEPTION_NONE
    }
    /// 判断当前`cte`是否为`next`节点的父节点（除了父节点，还有兄弟节点的关系可能）
    fn is_mdb_parent_of(&self, next: &Self) -> bool {
        if self.cteMDBNode.get_mdbRevocable() == 0 {
            return false;
        }
        if !same_region_as(&self.capability, &next.capability) {
            return false;
        }
        match self.capability.get_tag() {
            cap_tag::cap_endpoint_cap => {
                assert_eq!(next.capability.get_tag(), cap_tag::cap_endpoint_cap);
                let badge = cap::cap_endpoint_cap(&self.capability).get_capEPBadge();
                if badge == 0 {
                    return true;
                }
                badge == cap::cap_endpoint_cap(&next.capability).get_capEPBadge()
                    && next.cteMDBNode.get_mdbFirstBadged() == 0
            }
            cap_tag::cap_notification_cap => {
                assert_eq!(next.capability.get_tag(), cap_tag::cap_notification_cap);
                let badge = cap::cap_notification_cap(&self.capability).get_capNtfnBadge();
                if badge == 0 {
                    return true;
                }
                badge == cap::cap_notification_cap(&next.capability).get_capNtfnBadge()
                    && next.cteMDBNode.get_mdbFirstBadged() == 0
            }
            _ => true,
        }
    }

    /// 判断当前`cte`是否是能力派生树上的最后一个能力,如果`prev`与当前指向对象，则当前`cte`不是最后一个`cap`
    /// 如果`cte`的`next`是当前`cte`派生出来的能力，则当前`cte`也不是最后一个`cap`
    pub fn is_final_cap(&self) -> bool {
        let mdb = &self.cteMDBNode;
        let prev_is_same_obj = if mdb.get_mdbPrev() == 0 {
            false
        } else {
            let prev = convert_to_type_ref::<cte_t>(mdb.get_mdbPrev() as usize);
            same_object_as(&prev.capability, &self.capability)
        };

        if prev_is_same_obj {
            return false;
        }
        if mdb.get_mdbNext() == 0 {
            true
        } else {
            let next = convert_to_type_ref::<cte_t>(mdb.get_mdbNext() as usize);
            !same_object_as(&self.capability, &next.capability)
        }
    }

    pub fn is_long_running_delete(&self) -> bool {
        if self.capability.get_tag() == cap_tag::cap_null_cap || !self.is_final_cap() {
            return false;
        }
        matches!(
            self.capability.get_tag(),
            cap_tag::cap_thread_cap | cap_tag::cap_zombie_cap | cap_tag::cap_cnode_cap
        )
    }

    ///清除`cte slot`中的`capability`
    /// 因为涉及到几个函数之间的来回调用，不太好理解，所以用一个`CNode cap`删除的例子来帮助理解，
    /// 假设现在有一个`CNode`的`slot`要执行`delete_all(true)`，会先调用`finalise(true)`，在`finaliseCap`中，
    /// 会将`cnode_cap`设置为`zombie_cap`，然后进入`reduce_zombie`,
    /// `reduce_zombie`会调用最后一个`slot`的`delete_all(false)`函数，然后再次进入`finalise(false)`
    /// 假设最后一个`slot`存储的`cap`为一个二级`cnode_cap`，则会在`finaliseCap`中生成一个新的`zombie_cap`，
    /// 之后再次进入`reduce_zombie(false)`，在其中进入`else`分支，
    /// 执行`cteswap`将二级`cnode_cap`中的第一个`cap`与二级`cnode_cap`进行交换，使得二级`cnode_cap`指向自身，变成`cyclicZombie`。
    /// 然后继续清除即可。至于二级`cnode_cap`其实无法被清除。
    unsafe fn finalise(&mut self, immediate: bool) -> finaliseSlot_ret {
        let mut ret = finaliseSlot_ret::default();
        while self.capability.get_tag() != cap_tag::cap_null_cap {
            let fc_ret = finaliseCap(&self.capability, self.is_final_cap(), false);
            if cap_removable(&fc_ret.remainder, self) {
                ret.status = exception_t::EXCEPTION_NONE;
                ret.success = true;
                ret.cleanupInfo = fc_ret.cleanupInfo;
                return ret;
            }
            self.capability = fc_ret.clone().remainder;
            if !immediate && capCyclicZombie(&fc_ret.remainder, self) {
                ret.status = exception_t::EXCEPTION_NONE;
                ret.success = false;
                ret.cleanupInfo = fc_ret.cleanupInfo;
                return ret;
            }
            let status = self.reduce_zombie(immediate);
            if exception_t::EXCEPTION_NONE != status {
                ret.status = status;
                ret.success = false;
                ret.cleanupInfo = cap_null_cap::new().unsplay();
                return ret;
            }

            let status = preemptionPoint();
            if exception_t::EXCEPTION_NONE != status {
                ret.status = status;
                ret.success = false;
                ret.cleanupInfo = cap_null_cap::new().unsplay();
                return ret;
            }
        }
        ret
    }

    /// 将当前的`cte slot`中的能力清除，因为可能是`cnode_cap`或者`tcb_cap`，其中都可以存储多个`cap`，
    /// 所以可能顺带将存储的`cap`也清除掉
    pub fn delete_all(&mut self, exposed: bool) -> exception_t {
        let fs_ret = unsafe { self.finalise(exposed) };
        if fs_ret.status != exception_t::EXCEPTION_NONE {
            return fs_ret.status;
        }
        if exposed || fs_ret.success {
            self.set_empty(&fs_ret.cleanupInfo);
        }
        exception_t::EXCEPTION_NONE
    }

    /// 将当前的`cte slot`中的能力清除,要求`cap`是可删除的
    pub fn delete_one(&mut self) {
        if self.capability.get_tag() != cap_tag::cap_null_cap {
            let fc_ret = unsafe { finaliseCap(&self.capability, self.is_final_cap(), true) };
            assert!(
                cap_removable(&fc_ret.remainder, self)
                    && fc_ret.cleanupInfo.get_tag() == cap_tag::cap_null_cap
            );
            self.set_empty(&cap_null_cap::new().unsplay());
        }
    }

    /// 将当前`slot`从`capability derivation tree`中删除
    fn set_empty(&mut self, cleanup_info: &cap) {
        if self.capability.get_tag() != cap_tag::cap_null_cap {
            let mdb = &self.cteMDBNode;
            let prev_addr = mdb.get_mdbPrev() as usize;
            let next_addr = mdb.get_mdbNext() as usize;
            if prev_addr != 0 {
                let prev_node = convert_to_mut_type_ref::<cte_t>(prev_addr);
                prev_node.cteMDBNode.set_mdbNext(next_addr as u64);
            }

            if next_addr != 0 {
                let next_node = convert_to_mut_type_ref::<cte_t>(next_addr);
                next_node.cteMDBNode.set_mdbPrev(prev_addr as u64);
                let first_badged = ((next_node.cteMDBNode.get_mdbFirstBadged() != 0)
                    || (mdb.get_mdbFirstBadged() != 0)) as usize;
                next_node.cteMDBNode.set_mdbFirstBadged(first_badged as u64);
            }
            self.capability = cap_null_cap::new().unsplay();
            self.cteMDBNode = mdb_node {
                0: Bitfield { arr: [0; 2usize] },
            };
            unsafe { post_cap_deletion(cleanup_info) };
        }
    }

    /// 每次删除`zombie cap`中的最后一个`capability`,用于删除unremovable的capability。
    fn reduce_zombie(&mut self, immediate: bool) -> exception_t {
        assert_eq!(self.capability.get_tag(), cap_tag::cap_zombie_cap);
        let self_ptr = self as *mut cte_t as usize;
        let ptr = cap::cap_zombie_cap(&self.capability).get_zombie_ptr();
        let n = cap::cap_zombie_cap(&self.capability).get_zombie_number();
        let zombie_type = cap::cap_zombie_cap(&self.capability).get_capZombieType();
        assert!(n > 0);
        if immediate {
            let end_slot = unsafe { &mut *((ptr as *mut cte_t).add(n - 1)) };
            let status = end_slot.delete_all(false);
            if status != exception_t::EXCEPTION_NONE {
                return status;
            }
            match self.capability.get_tag() {
                cap_tag::cap_null_cap => {
                    return exception_t::EXCEPTION_NONE;
                }
                cap_tag::cap_zombie_cap => {
                    let ptr2 = cap::cap_zombie_cap(&self.capability).get_zombie_ptr();
                    if ptr == ptr2
                        && cap::cap_zombie_cap(&self.capability).get_zombie_number() == n
                        && cap::cap_zombie_cap(&self.capability).get_capZombieType() == zombie_type
                    {
                        assert_eq!(end_slot.capability.get_tag(), cap_tag::cap_null_cap);
                        cap::cap_zombie_cap(&self.capability).set_zombie_number(n - 1);
                    } else {
                        assert!(ptr2 == self_ptr && ptr != self_ptr);
                    }
                }
                _ => {
                    panic!("Expected recursion to result in Zombie.")
                }
            }
        } else {
            assert_ne!(ptr, self_ptr);
            let next_slot = convert_to_mut_type_ref::<cte_t>(ptr);
            let cap1 = next_slot.capability.clone();
            let cap2 = self.capability.clone();
            cte_swap(&cap1, next_slot, &cap2, self);
        }
        exception_t::EXCEPTION_NONE
    }

    #[cfg(target_arch = "riscv64")]
    #[inline]
    fn get_volatile_value(&self) -> usize {
        unsafe {
            let raw_value = ptr::read_volatile((self.get_ptr() + 24) as *const usize);
            let mut value = ((raw_value >> 2) & MASK!(37)) << 2;
            if (value & (1usize << 38)) != 0 {
                value |= 0xffffff8000000000;
            }
            value
        }
    }

    #[cfg(target_arch = "aarch64")]
    #[inline]
    fn get_volatile_value(&self) -> usize {
        unsafe {
            let raw_value = ptr::read_volatile((self.get_ptr() + 24) as *const usize);
            let mut value = ((raw_value >> 2) & MASK!(46)) << 2;
            if (value & (1usize << 46)) != 0 {
                value |= 0xffffff8000000000;
            }
            value
        }
    }

    // 撤销当前`cte`中的`capability`
    #[inline]
    pub fn revoke(&mut self) -> exception_t {
        while let Some(cte) = convert_to_option_mut_type_ref::<cte_t>(self.get_volatile_value()) {
            if !self.is_mdb_parent_of(cte) {
                break;
            }

            let mut status = cte.delete_all(true);
            if status != exception_t::EXCEPTION_NONE {
                return status;
            }

            status = unsafe { preemptionPoint() };
            if status != exception_t::EXCEPTION_NONE {
                return status;
            }
        }
        exception_t::EXCEPTION_NONE
    }
}

/// 将一个cap插入slot中并维护能力派生树
///
/// 将一个new_cap插入到dest slot中并作为src slot的派生子节点插入派生树中
pub fn cte_insert(new_cap: &cap, src_slot: &mut cte_t, dest_slot: &mut cte_t) {
    let srcMDB = &mut src_slot.cteMDBNode;
    let srcCap = &(src_slot.capability.clone());
    let mut newMDB = srcMDB.clone();
    let newCapIsRevocable = is_cap_revocable(new_cap, srcCap);
    newMDB.set_mdbPrev(src_slot as *const cte_t as u64);
    newMDB.set_mdbRevocable(newCapIsRevocable as u64);
    newMDB.set_mdbFirstBadged(newCapIsRevocable as u64);

    /* Haskell error: "cteInsert to non-empty destination" */
    assert_eq!(dest_slot.capability.get_tag(), cap_tag::cap_null_cap);
    /* Haskell error: "cteInsert: mdb entry must be empty" */
    assert!(dest_slot.cteMDBNode.get_mdbNext() == 0 && dest_slot.cteMDBNode.get_mdbPrev() == 0);

    setUntypedCapAsFull(srcCap, new_cap, src_slot);

    dest_slot.capability = new_cap.clone();
    dest_slot.cteMDBNode = newMDB.clone();
    src_slot
        .cteMDBNode
        .set_mdbNext(dest_slot as *const cte_t as u64);
    if newMDB.get_mdbNext() != 0 {
        let cte_ref = convert_to_mut_type_ref::<cte_t>(newMDB.get_mdbNext() as usize);
        cte_ref
            .cteMDBNode
            .set_mdbPrev(dest_slot as *const cte_t as u64);
    }
}

/// insert a new cap to slot, set parent's next is slot.
pub fn insert_new_cap(parent: &mut cte_t, slot: &mut cte_t, capability: &cap) {
    let next = parent.cteMDBNode.get_mdbNext() as usize;
    slot.capability = capability.clone();
    slot.cteMDBNode = mdb_node::new(next as u64, 1u64, 1u64, parent as *const cte_t as u64);
    if next != 0 {
        let next_ref = convert_to_mut_type_ref::<cte_t>(next);
        next_ref.cteMDBNode.set_mdbPrev(slot as *const cte_t as u64);
    }
    parent.cteMDBNode.set_mdbNext(slot as *const cte_t as u64);
}

/// 将一个cap插入slot中并删除原节点
///
/// 将一个new_cap插入到dest slot中并作为替代src slot在派生树中的位置
pub fn cte_move(new_cap: &cap, src_slot: &mut cte_t, dest_slot: &mut cte_t) {
    /* Haskell error: "cteInsert to non-empty destination" */
    assert_eq!(dest_slot.capability.get_tag(), cap_tag::cap_null_cap);
    /* Haskell error: "cteInsert: mdb entry must be empty" */
    assert!(dest_slot.cteMDBNode.get_mdbNext() == 0 && dest_slot.cteMDBNode.get_mdbPrev() == 0);
    let mdb = src_slot.cteMDBNode.clone();
    dest_slot.capability = new_cap.clone();
    src_slot.capability = cap_null_cap::new().unsplay();
    dest_slot.cteMDBNode = mdb.clone();
    src_slot.cteMDBNode = mdb_node::new(0, 0, 0, 0);

    let prev_ptr = mdb.clone().get_mdbPrev() as usize;
    if prev_ptr != 0 {
        let prev_ref = convert_to_mut_type_ref::<cte_t>(prev_ptr);
        prev_ref
            .cteMDBNode
            .set_mdbNext(dest_slot as *const cte_t as u64);
    }
    let next_ptr = mdb.get_mdbNext() as usize;
    if next_ptr != 0 {
        let next_ref = convert_to_mut_type_ref::<cte_t>(next_ptr);
        next_ref
            .cteMDBNode
            .set_mdbPrev(dest_slot as *const cte_t as u64);
    }
}

/// 交换两个slot，并将新的cap数据填入
pub fn cte_swap(cap1: &cap, slot1: &mut cte_t, cap2: &cap, slot2: &mut cte_t) {
    let mdb1 = slot1.cteMDBNode.clone();
    let mdb2 = slot2.cteMDBNode.clone();
    {
        let prev_ptr = mdb1.get_mdbPrev() as usize;
        if prev_ptr != 0 {
            convert_to_mut_type_ref::<cte_t>(prev_ptr)
                .cteMDBNode
                .set_mdbNext(slot2 as *const cte_t as u64);
        }
        let next_ptr = mdb1.get_mdbNext() as usize;
        if next_ptr != 0 {
            convert_to_mut_type_ref::<cte_t>(next_ptr)
                .cteMDBNode
                .set_mdbPrev(slot2 as *const cte_t as u64);
        }
    }

    slot1.capability = cap2.clone();
    //FIXME::result not right due to compiler

    slot2.capability = cap1.clone();
    slot1.cteMDBNode = mdb2.clone();
    slot2.cteMDBNode = mdb1.clone();
    {
        let prev_ptr = mdb2.get_mdbPrev() as usize;
        if prev_ptr != 0 {
            convert_to_mut_type_ref::<cte_t>(prev_ptr)
                .cteMDBNode
                .set_mdbNext(slot1 as *const cte_t as u64);
        }
        let next_ptr = mdb2.get_mdbNext() as usize;
        if next_ptr != 0 {
            convert_to_mut_type_ref::<cte_t>(next_ptr)
                .cteMDBNode
                .set_mdbPrev(slot1 as *const cte_t as u64);
        }
    }
}

/// 判断当前`cap`能否被删除，只有`CNode Capability`能够做到`slot=z_slot`，且n==1意味着是`tcb`初始分配的`CNode`。
#[inline]
fn cap_removable(capability: &cap, slot: *mut cte_t) -> bool {
    match capability.get_tag() {
        cap_tag::cap_null_cap => true,
        cap_tag::cap_zombie_cap => {
            let n = cap::cap_zombie_cap(capability).get_zombie_number();
            let ptr = cap::cap_zombie_cap(capability).get_zombie_ptr();
            let z_slot = ptr as *mut cte_t;
            n == 0 || (n == 1 && slot == z_slot)
        }
        _ => {
            panic!("Invalid cap type , finaliseCap should only return Zombie or NullCap");
        }
    }
}

/// 如果`srcCap`和`newCap`都是`UntypedCap`，并且指向同一块内存，内存大小也相同，就将`srcCap`记录为没有剩余空间。
/// 自我认为是防止同一块内存空间被分配两次
fn setUntypedCapAsFull(srcCap: &cap, newCap: &cap, srcSlot: &mut cte_t) {
    if srcCap.get_tag() == cap_tag::cap_untyped_cap && newCap.get_tag() == cap_tag::cap_untyped_cap
    {
        assert_eq!(srcSlot.capability.get_tag(), cap_tag::cap_untyped_cap);
        if cap::cap_untyped_cap(srcCap).get_capPtr() == cap::cap_untyped_cap(newCap).get_capPtr()
            && cap::cap_untyped_cap(srcCap).get_capBlockSize()
                == cap::cap_untyped_cap(newCap).get_capBlockSize()
        {
            cap::cap_untyped_cap(&srcSlot.capability).set_capFreeIndex(MAX_FREE_INDEX(
                cap::cap_untyped_cap(srcCap).get_capBlockSize() as usize,
            ) as u64);
        }
    }
}

/// 从cspace寻址特定的slot
///
/// 从给定的cnode、cap index、和depth中找到对应cap的slot，成功则返回slot指针，失败返回找到的最深的cnode
///
/// Parse cap_ptr ,get a capbility from cnode.
#[allow(unreachable_code)]
pub fn resolve_address_bits(
    node_cap: &cap_cnode_cap,
    cap_ptr: usize,
    _n_bits: usize,
) -> resolveAddressBits_ret_t {
    let mut ret = resolveAddressBits_ret_t::default();
    let mut n_bits = _n_bits;
    ret.bitsRemaining = n_bits;
    let mut nodeCap = node_cap.clone();

    if unlikely(nodeCap.clone().unsplay().get_tag() != cap_tag::cap_cnode_cap) {
        ret.status = exception_t::EXCEPTION_LOOKUP_FAULT;
        return ret;
    }

    loop {
        let radixBits = nodeCap.get_capCNodeRadix() as usize;
        let guardBits = nodeCap.get_capCNodeGuardSize() as usize;
        let levelBits = radixBits + guardBits;
        assert_ne!(levelBits, 0);
        let capGuard = nodeCap.get_capCNodeGuard() as usize;
        let guard = (cap_ptr >> ((n_bits - guardBits) & MASK!(wordRadix))) & MASK!(guardBits);
        if unlikely(guardBits > n_bits || guard != capGuard) {
            ret.status = exception_t::EXCEPTION_LOOKUP_FAULT;
            return ret;
        }
        if unlikely(levelBits > n_bits) {
            ret.status = exception_t::EXCEPTION_LOOKUP_FAULT;
            return ret;
        }
        let offset = (cap_ptr >> (n_bits - levelBits)) & MASK!(radixBits);
        let slot = unsafe { (nodeCap.get_capCNodePtr() as *mut cte_t).add(offset) };

        if likely(n_bits == levelBits) {
            ret.slot = slot;
            ret.bitsRemaining = 0;
            return ret;
        }
        n_bits -= levelBits;
        nodeCap = unsafe { cap::cap_cnode_cap(&(*slot).capability).clone() };
        if unlikely(nodeCap.clone().unsplay().get_tag() != cap_tag::cap_cnode_cap) {
            ret.slot = slot;
            ret.bitsRemaining = n_bits;
            return ret;
        }
    }
    panic!("UNREACHABLE");
}
