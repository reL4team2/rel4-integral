# 0 "/workspace/rel4test/rel4_kernel/kernel/src/arch/riscv/head.S"
# 0 "<built-in>"
# 0 "<command-line>"
# 1 "/usr/include/stdc-predef.h" 1 3 4
# 0 "<command-line>" 2
# 1 "/workspace/rel4test/rel4_kernel/kernel/src/arch/riscv/head.S"
# 9 "/workspace/rel4test/rel4_kernel/kernel/src/arch/riscv/head.S"
# 1 "/workspace/rel4test/rel4_kernel/kernel/include/assembler.h" 1






       
# 10 "/workspace/rel4test/rel4_kernel/kernel/src/arch/riscv/head.S" 2

.section .boot.text, "ax"
.global _start
.extern init_kernel
.extern kernel_stack_alloc
.extern __global_pointer$
.extern restore_user_context





_start:
  fence.i
.option push
.option norelax
1:auipc gp, %pcrel_hi(__global_pointer$)
  addi gp, gp, %pcrel_lo(1b)
.option pop
  la sp, (kernel_stack_alloc + (1 << (12)))
  csrw sscratch, x0
# 44 "/workspace/rel4test/rel4_kernel/kernel/src/arch/riscv/head.S"
  jal init_kernel
# 54 "/workspace/rel4test/rel4_kernel/kernel/src/arch/riscv/head.S"
  la ra, restore_user_context
  jr ra
