.section .text

.global trap_entry
.extern c_handle_syscall
.extern c_handle_fastpath_reply_recv
.extern c_handle_fastpath_call
.extern c_handle_interrupt
.extern c_handle_exception

trap_entry:
  csrrw t0, sscratch, t0
  sd ra, (0*(64 / 8))(t0)
  sd sp, (1*(64 / 8))(t0)
  sd gp, (2*(64 / 8))(t0)
  sd tp, (3*(64 / 8))(t0)
  sd t1, (5*(64 / 8))(t0)
  sd t2, (6*(64 / 8))(t0)
  sd s0, (7*(64 / 8))(t0)
  sd s1, (8*(64 / 8))(t0)
  sd a0, (9*(64 / 8))(t0)
  sd a1, (10*(64 / 8))(t0)
  sd a2, (11*(64 / 8))(t0)
  sd a3, (12*(64 / 8))(t0)
  sd a4, (13*(64 / 8))(t0)
  sd a5, (14*(64 / 8))(t0)
  sd a6, (15*(64 / 8))(t0)
  sd a7, (16*(64 / 8))(t0)
  sd s2, (17*(64 / 8))(t0)
  sd s3, (18*(64 / 8))(t0)
  sd s4, (19*(64 / 8))(t0)
  sd s5, (20*(64 / 8))(t0)
  sd s6, (21*(64 / 8))(t0)
  sd s7, (22*(64 / 8))(t0)
  sd s8, (23*(64 / 8))(t0)
  sd s9, (24*(64 / 8))(t0)
  sd s10, (25*(64 / 8))(t0)
  sd s11, (26*(64 / 8))(t0)
  sd t3, (27*(64 / 8))(t0)
  sd t4, (28*(64 / 8))(t0)
  sd t5, (29*(64 / 8))(t0)
  sd t6, (30*(64 / 8))(t0)
  csrr x1, sscratch
  sd x1, (4*(64 / 8))(t0)
  csrr x1, sstatus
  sd x1, (32*(64 / 8))(t0)
  csrr s0, scause
  sd s0, (31*(64 / 8))(t0)

.option push
.option norelax
  la gp, __global_pointer$
.option pop
  la sp, (kernel_stack_alloc + (1ul << (12)))
  csrr x1, sepc
  sd x1, (33*(64 / 8))(t0)
  bltz s0, interrupt
  li s4, 8
  bne s0, s4, exception

handle_syscall:
  addi x1, x1, 4
  sd x1, (34*(64 / 8))(t0)
  li t3, -1
  bne a7, t3, 1f
  j c_handle_fastpath_call
1:
  li t3, -2
  bne a7, t3, 2f
  j c_handle_fastpath_reply_recv
2:
  mv a2, a7
  j c_handle_syscall

exception:
  sd x1, (34*(64 / 8))(t0)
  j c_handle_exception

interrupt:
  sd x1, (34*(64 / 8))(t0)
  j c_handle_interrupt
