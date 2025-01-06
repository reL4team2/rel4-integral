
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
  la sp, (kernel_stack_alloc + (1ul << (12)))
  csrw sscratch, x0
  jal init_kernel
  la ra, restore_user_context
  jr ra
