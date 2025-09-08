#![feature(core_intrinsics)]
#![no_std]
#![no_main]
#![allow(internal_features)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(clippy::clone_on_copy)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]

#[macro_use]
extern crate rel4_utils;

pub mod capability;
mod cte;
mod structures;

/// 需要外部实现的接口
pub mod deps;
/// 暴露给外部的接口
pub mod interface;

/// 兼容c风格的接口，后续会删除
pub mod compatibility;

pub mod arch;

#[cfg(test)]
mod tests {
    use capability::same_object_as;
    use core::arch::global_asm;
    use cte::{cte_insert, cte_move, cte_swap, cte_t, insert_new_cap, resolve_address_bits};
    use riscv::register::{stvec, utvec::TrapMode};
    use sel4_common::structures_gen::cap_tag;
    use sel4_common::structures_gen::mdb_node;
    use sel4_common::structures_gen::{
        cap, cap_asid_control_cap, cap_asid_pool_cap, cap_cnode_cap, cap_frame_cap,
        cap_page_table_cap,
    };
    use sel4_common::{arch::shutdown, println, utils::convert_to_mut_type_ref};
    global_asm!(include_str!("entry.asm"));

    use super::*;

    #[test_case]
    pub fn same_object_as_test() {
        use sel4_common::structures_gen::cap_cnode_cap;

        println!("-----------------------------------");
        println!("Entering same_object_as_test case");
        let cap1 = cap_cnode_cap::new(1, 1, 1, 1).unsplay();
        let cap3 = cap_cnode_cap::new(1, 1, 2, 1).unsplay();
        let mdb = mdb_node::new(0, 0, 0, 0);
        let mut cte1 = cte_t {
            capability: cap1,
            cteMDBNode: mdb,
        };
        let cap2 = cte1.derive_cap(&cap3).capability;
        assert_eq!(same_object_as(&cte1.capability, &cap2), false);
        assert_eq!(same_object_as(&cap2, &cap3), true);
        println!("Test same_object_as_test passed");
        println!("-----------------------------------");
    }

    #[test_case]
    pub fn cte_insert_test() {
        use sel4_common::structures_gen::{cap_asid_control_cap, cap_domain_cap, cap_null_cap};

        println!("-----------------------------------");
        println!("Entering cte_insert_test case");
        let cap1 = cap_asid_control_cap::new().unsplay();
        let cap2 = cap_domain_cap::new().unsplay();
        let mut cte1 = cte_t {
            capability: cap_null_cap::new().unsplay(),
            cteMDBNode: mdb_node::new(0, 0, 0, 0),
        };
        let mut cte2 = cte_t {
            capability: cap_null_cap::new().unsplay(),
            cteMDBNode: mdb_node::new(0, 0, 0, 0),
        };
        let mut cte3 = cte_t {
            capability: cap_null_cap::new().unsplay(),
            cteMDBNode: mdb_node::new(0, 0, 0, 0),
        };
        cte_insert(&cap1, &mut cte1, &mut cte2);
        cte_insert(&cap2, &mut cte2, &mut cte3);
        assert_eq!(cte2.capability.get_tag(), cap_tag::cap_asid_control_cap);
        assert_eq!(cte3.capability.get_tag(), cap_tag::cap_domain_cap);
        assert_eq!(
            cte1.cteMDBNode.get_mdbNext(),
            &mut cte2 as *mut cte_t as u64
        );
        assert_eq!(
            cte2.cteMDBNode.get_mdbNext(),
            &mut cte3 as *mut cte_t as u64
        );
        assert_eq!(
            cte2.cteMDBNode.get_mdbPrev(),
            &mut cte1 as *mut cte_t as u64
        );
        assert_eq!(
            cte3.cteMDBNode.get_mdbPrev(),
            &mut cte2 as *mut cte_t as u64
        );
        println!("Test cte_insert_test passed");
    }

    #[test_case]
    pub fn cte_move_test() {
        use sel4_common::structures_gen::{
            cap_asid_control_cap, cap_domain_cap, cap_irq_control_cap, cap_null_cap,
        };

        println!("-----------------------------------");
        println!("Entering cte_move_test case");
        let cap1 = cap_asid_control_cap::new().unsplay();
        let cap2 = cap_domain_cap::new().unsplay();
        let cap3 = cap_irq_control_cap::new().unsplay();
        let mut cte1 = cte_t {
            capability: cap_null_cap::new().unsplay(),
            cteMDBNode: mdb_node::new(0, 0, 0, 0),
        };
        let mut cte2 = cte_t {
            capability: cap_null_cap::new().unsplay(),
            cteMDBNode: mdb_node::new(0, 0, 0, 0),
        };
        let mut cte3 = cte_t {
            capability: cap_null_cap::new().unsplay(),
            cteMDBNode: mdb_node::new(0, 0, 0, 0),
        };
        let mut cte4 = cte_t {
            capability: cap_null_cap::new().unsplay(),
            cteMDBNode: mdb_node::new(0, 0, 0, 0),
        };
        cte_insert(&cap1, &mut cte1, &mut cte2);
        cte_insert(&cap2, &mut cte2, &mut cte3);
        assert_eq!(
            cte1.cteMDBNode.get_mdbNext(),
            &mut cte2 as *mut cte_t as u64
        );
        assert_eq!(
            cte2.cteMDBNode.get_mdbNext(),
            &mut cte3 as *mut cte_t as u64
        );
        assert_eq!(
            cte2.cteMDBNode.get_mdbPrev(),
            &mut cte1 as *mut cte_t as u64
        );
        assert_eq!(
            cte3.cteMDBNode.get_mdbPrev(),
            &mut cte2 as *mut cte_t as u64
        );
        cte_move(&cap3, &mut cte2, &mut cte4);
        assert_eq!(cte4.capability.get_tag(), cap_tag::cap_irq_control_cap);
        assert_eq!(
            cte4.cteMDBNode.get_mdbNext(),
            &mut cte3 as *mut cte_t as u64
        );
        assert_eq!(
            cte4.cteMDBNode.get_mdbPrev(),
            &mut cte1 as *mut cte_t as u64
        );
        assert_eq!(
            cte1.cteMDBNode.get_mdbNext(),
            &mut cte4 as *mut cte_t as u64
        );
        assert_eq!(
            cte3.cteMDBNode.get_mdbPrev(),
            &mut cte4 as *mut cte_t as u64
        );
        assert_ne!(
            cte1.cteMDBNode.get_mdbNext(),
            &mut cte2 as *mut cte_t as u64
        );
        assert_ne!(
            cte3.cteMDBNode.get_mdbPrev(),
            &mut cte2 as *mut cte_t as u64
        );
        assert_ne!(
            cte2.cteMDBNode.get_mdbNext(),
            &mut cte3 as *mut cte_t as u64
        );
        assert_ne!(
            cte2.cteMDBNode.get_mdbPrev(),
            &mut cte1 as *mut cte_t as u64
        );
        println!("Test cte_move_test passed");
    }

    #[test_case]
    pub fn cte_swap_test() {
        use sel4_common::structures_gen::{cap_asid_control_cap, cap_domain_cap, cap_null_cap};

        println!("-----------------------------------");
        println!("Entering cte_swap_test case");
        let cap1 = cap_asid_control_cap::new().unsplay();
        let cap2 = cap_domain_cap::new().unsplay();
        let mut cte1 = cte_t {
            capability: cap_null_cap::new().unsplay(),
            cteMDBNode: mdb_node::new(0, 0, 0, 0),
        };
        let mut cte2 = cte_t {
            capability: cap_null_cap::new().unsplay(),
            cteMDBNode: mdb_node::new(0, 0, 0, 0),
        };

        let mut cte3 = cte_t {
            capability: cap_null_cap::new().unsplay(),
            cteMDBNode: mdb_node::new(0, 0, 0, 0),
        };
        let mut cte4 = cte_t {
            capability: cap_null_cap::new().unsplay(),
            cteMDBNode: mdb_node::new(0, 0, 0, 0),
        };

        cte_insert(&cap1, &mut cte1, &mut cte2);
        cte_insert(&cap2, &mut cte3, &mut cte4);
        assert_eq!(
            cte1.cteMDBNode.get_mdbNext(),
            &mut cte2 as *mut cte_t as u64
        );
        assert_eq!(
            cte2.cteMDBNode.get_mdbPrev(),
            &mut cte1 as *mut cte_t as u64
        );
        assert_eq!(
            cte3.cteMDBNode.get_mdbNext(),
            &mut cte4 as *mut cte_t as u64
        );
        assert_eq!(
            cte4.cteMDBNode.get_mdbPrev(),
            &mut cte3 as *mut cte_t as u64
        );
        cte_swap(&cap1, &mut cte2, &cap2, &mut cte4);
        assert_eq!(cte2.capability.get_tag(), cap_tag::cap_domain_cap);
        assert_eq!(cte4.capability.get_tag(), cap_tag::cap_asid_control_cap);
        assert_eq!(
            cte4.cteMDBNode.get_mdbPrev(),
            &mut cte1 as *mut cte_t as u64
        );
        assert_eq!(
            cte1.cteMDBNode.get_mdbNext(),
            &mut cte4 as *mut cte_t as u64
        );
        assert_eq!(
            cte2.cteMDBNode.get_mdbPrev(),
            &mut cte3 as *mut cte_t as u64
        );
        assert_eq!(
            cte3.cteMDBNode.get_mdbNext(),
            &mut cte2 as *mut cte_t as u64
        );

        println!("Test cte_swap_test passed");
    }

    #[test_case]
    pub fn insert_new_cap_test() {
        use sel4_common::structures_gen::{cap_asid_control_cap, cap_domain_cap, cap_null_cap};

        println!("-----------------------------------");
        println!("Entering insert_new_cap_test case");
        let cap1 = cap_asid_control_cap::new().unsplay();
        let cap2 = cap_domain_cap::new().unsplay();
        let mut cte1 = cte_t {
            capability: cap_null_cap::new().unsplay(),
            cteMDBNode: mdb_node::new(0, 0, 0, 0),
        };
        let mut cte2 = cte_t {
            capability: cap_null_cap::new().unsplay(),
            cteMDBNode: mdb_node::new(0, 0, 0, 0),
        };
        let mut cte3 = cte_t {
            capability: cap_null_cap::new().unsplay(),
            cteMDBNode: mdb_node::new(0, 0, 0, 0),
        };
        cte_insert(&cap1, &mut cte1, &mut cte2);
        assert_eq!(cte2.capability.get_tag(), cap_tag::cap_asid_control_cap);
        assert_eq!(
            cte1.cteMDBNode.get_mdbNext(),
            &mut cte2 as *mut cte_t as u64
        );
        assert_eq!(
            cte2.cteMDBNode.get_mdbPrev(),
            &mut cte1 as *mut cte_t as u64
        );
        insert_new_cap(&mut cte1, &mut cte3, &cap2);
        assert_eq!(
            cte1.cteMDBNode.get_mdbNext(),
            &mut cte3 as *mut cte_t as u64
        );
        assert_eq!(
            cte3.cteMDBNode.get_mdbNext(),
            &mut cte2 as *mut cte_t as u64
        );
        assert_eq!(
            cte2.cteMDBNode.get_mdbPrev(),
            &mut cte3 as *mut cte_t as u64
        );
        assert_eq!(
            cte3.cteMDBNode.get_mdbPrev(),
            &mut cte1 as *mut cte_t as u64
        );
        println!("Test insert_new_cap_test passed");
    }

    #[test_case]
    pub fn resolve_address_bits_test() {
        use sel4_common::structures_gen::{cap_cnode_cap, cap_domain_cap, cap_null_cap};

        println!("-----------------------------------");
        println!("Entering resolve_address_bits_test case");
        //cap_ptr structure:
        // guard1(2 bits)|offset1(3 bits)|guard2(2 bits)|offset2(3 bits)
        let buffer: [u8; 1024] = [0; 1024];
        let guardSize = 2;
        let guard1 = 2;
        let guard2 = 3;
        let cap1 = cap_cnode_cap::new(guard1, guardSize, 3, buffer.as_ptr() as u64);
        let cap2 = cap_cnode_cap::new(guard2, guardSize, 3, buffer.as_ptr() as u64);
        let mut cte1 = cte_t {
            capability: cap_null_cap::new().unsplay(),
            cteMDBNode: mdb_node::new(0, 0, 0, 0),
        };
        let cap3 = cap_domain_cap::new().unsplay();
        let idx: u64 = 2;
        let cap_ptr = (guard1 << 8) | (idx << 5) | (guard2 << 3) | idx;
        insert_new_cap(
            &mut cte1,
            convert_to_mut_type_ref((cap1.get_capCNodePtr() + idx * 32) as usize),
            &cap2.clone().unsplay(),
        );
        insert_new_cap(
            &mut cte1,
            convert_to_mut_type_ref((cap2.get_capCNodePtr() + idx * 32) as usize),
            &cap3,
        );
        let res_ret = resolve_address_bits(&cap1, cap_ptr as usize, 10);

        let ret_cap = unsafe { &(*(res_ret.slot)).capability };
        assert_eq!(ret_cap.get_tag(), cap_tag::cap_domain_cap);
        println!("Test resolve_address_bits_test passed");
    }

    #[test_case]
    pub fn cap_t_create_happy_test() {
        use sel4_common::structures_gen::cap_cnode_cap;

        println!("-----------------------------------");
        println!("Entering cap_t_create_happy_test case");
        let cap1 = cap_cnode_cap::new(1, 1, 1, 1);
        assert_eq!(cap1.clone().unsplay().get_tag(), cap_tag::cap_cnode_cap);
        assert_eq!(cap1.get_capCNodeGuardSize(), 1);
        println!("Test cap_t_create_happy_test passed");
    }

    #[test_case]
    pub fn slot_get_ptr_happy_case_test() {
        println!("-----------------------------------");
        println!("Entering slot_get_ptr_happy_case_test case");

        let mut slot = new_mock_slot(cap_tag::cap_cnode_cap);
        println!("Slot: {:?}", slot.get_ptr());

        let slot = &mut slot;
        println!("Slot: {:?}", slot.get_ptr());

        assert!(slot.get_ptr() == slot.get_ptr());

        println!("Test slot_get_ptr_happy_case_test passed");
    }

    #[test_case]
    pub fn shutdown_test() {
        println!("All Test Cases passed, shutdown");
        shutdown();
    }

    fn new_mock_slot(tag: u64) -> cte_t {
        match tag {
            cap_tag::cap_cnode_cap => {
                let capability = cap_cnode_cap::new(0, 0, 0, 0);
                cte_t {
                    capability: capability.unsplay(),
                    cteMDBNode: mdb_node::new(0, 0, 0, 0),
                }
            }
            cap_tag::cap_frame_cap => {
                let capability = cap_frame_cap::new(0, 0, 0, 0, 0, 0);
                cte_t {
                    capability: capability.unsplay(),
                    cteMDBNode: mdb_node::new(0, 0, 0, 0),
                }
            }
            cap_tag::cap_page_table_cap => {
                let capability = cap_page_table_cap::new(0, 0, 0, 0);
                cte_t {
                    capability: capability.unsplay(),
                    cteMDBNode: mdb_node::new(0, 0, 0, 0),
                }
            }
            cap_tag::cap_asid_control_cap => {
                let capability = cap_asid_control_cap::new();
                cte_t {
                    capability: capability.unsplay(),
                    cteMDBNode: mdb_node::new(0, 0, 0, 0),
                }
            }
            cap_tag::cap_asid_pool_cap => {
                let capability = cap_asid_pool_cap::new(0, 0);
                cte_t {
                    capability: capability.unsplay(),
                    cteMDBNode: mdb_node::new(0, 0, 0, 0),
                }
            }
            _ => panic!("Invalid cap tag"),
        }
    }

    #[panic_handler]
    fn panic(info: &core::panic::PanicInfo) -> ! {
        println!("{}", info);
        shutdown()
    }

    pub fn test_runner(tests: &[&dyn Fn()]) {
        println!("Running {} tests", tests.len());
        for test in tests {
            test();
        }
    }

    #[no_mangle]
    pub fn call_test_main() {
        extern "C" {
            fn trap_entry();
        }
        unsafe {
            stvec::write(trap_entry as usize, TrapMode::Direct);
        }
        crate::test_main();
    }
    #[no_mangle]
    pub fn c_handle_syscall() {
        unsafe {
            core::arch::asm!("sret");
        }
    }
}
