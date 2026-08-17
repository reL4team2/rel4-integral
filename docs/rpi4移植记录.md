# AArch64 RPi4 移植与 Bug 修复记录

## 概述

从 QEMU virt 平台向 Raspberry Pi 4B (BCM2711) 移植 rel4 内核，
修复了 6 个关键 bug，最终通过 sel4test 单核测试套件。

## Commit 总览

| Commit | 描述 |
|--------|------|
| `51e0b02` | add bcm2711 platform and miniuart driver |
| `765f44e` | fix linker scripts entry points |
| `11094ac` | fix rootserver boot bug |
| `35658b3` | fix page table attribute |
| `a23fd2c` | fix cache flush bug |
| `ff07a8e` | fix page map unmap bug |

涉及 19 个文件，新增 ~450 行，删除 ~50 行（不含新增文档文件）。

---

## Commit 1: `51e0b02` — add bcm2711 platform and miniuart driver

**文件:** 16 files changed, +397, -30

### 新增 BCM2711 平台配置

**`rel4_config/cfg/platform/bcm2711.yml`** (新增, +154)

Raspberry Pi 4B 完整平台配置：
- CPU: Cortex-A72, 54MHz 时钟
- 内存: `vmem_offset=0xffffff8000000000`, 可用物理内存 0x1000-0x3b400000 + 0x40000000-0x80000000
- 外设: UART(0xfe215000), GICv2_dist(0xff841000), GICv2_ctrl(0xff842000), local_intc(0xff800000)
- 中断: VTIMER/KERNEL_TIMER_IRQ=27, MAX_IRQ=216
- 内核: `USER_TOP=0xa0000000`, `ARM_PA_SIZE_BITS_44`, `ROOT_CNODE_SIZE_BITS=13`

### 新增 BCM2711 平台常量

**`sel4_common/src/platform/bcm2711.rs`** (新增, +55)
- 虚拟定时器 CNTV: `CONFIGURE_TIMER_FREQUENCY=54_000_000`
- 串口: MiniUart 绑定 `KDEV_BASE`
- `reset_timer` 用 `TVAL` 写 tick 值，含 `isb` 同步

### 平台模块分派

**`sel4_common/src/platform/mod.rs`**: 按 feature 分派：
- `platform_bcm2711` → `bcm2711`
- 否则 → `qemu_arm_virt`

### 新增 Mini UART 驱动

**`serial-impl/mini_uart/`** (新增, +91)
- AUX block 基址 0xfe215000，16550-like
- 波特率 115200 (div=543), 8-bit, 无中断
- `putchar` 自动 CR (`\n`→`\r\n`)
- 轮询 LSR 位（TX FIFO 空, RX ready）

### 构建系统

- `Cargo.toml/Cargo.lock`: 新增依赖
- `kernel/Cargo.toml`: `platform_bcm2711` feature
- `sel4_common/Cargo.toml`: `serial-impl-mini-uart` 依赖，`platform_bcm2711` feature
- `xtask/src/install.rs`: `bcm2711` → `kernel-settings-aarch64.cmake`
- `xtask/src/kernel.rs`: target `aarch64-unknown-none-softfloat`, `-DRPI4_MEMORY=2048`, 构建日志

---

## Commit 2: `765f44e` — fix linker scripts entry points

**文件:** `kernel/src/arch/aarch64/linker.ld.in` (+18, -4)

**问题:** ELF header segment 的物理地址默认等于虚拟地址，
elfloader 的 `elf_getMemoryBounds(phys=1)` 读到错误的物理范围，
DTB 重定位时写入非法地址导致崩溃。

**修复:**

1. **PHDRS 指令强制 header paddr=0x0:**
   ```
   PHDRS { elf_headers PT_LOAD FILEHDR PHDRS AT(0x0); }
   ```

2. **所有 output section 正确 LMA:** 每个 section 添加 `AT(ADDR(x) - KERNEL_OFFSET)`

3. **Fastpath 段支持:** 新增 `.vectors.fastpath_call`、`.vectors.fastpath_reply_recv`、`.vectors.text`

4. **4K 断点栈符号:** `.bss` 中新增 `_breakpoint_stack_bottom` / `_breakpoint_stack_top`

5. **丢弃无用段:** `.note.gnu.build-id`、`.comment`

---

## Commit 3: `11094ac` — fix rootserver boot bug

**文件:** `kernel/src/arch/aarch64/pg.rs` (+1, -1), `kernel/src/boot/root_server.rs` (+5)

### PTE index 掩码修正

**`kernel/src/arch/aarch64/pg.rs`**: `PTE` index 掩码从 `mask_bits!(9)` 修正为 `mask_bits!(PAGE_BITS)`。

### rootserver 对象内存清零

**症状:** 真机上 `cte_insert` 断言失败（目的 slot 非空），QEMU 正常。

**C 版对比** (`sel4test/kernel/src/kernel/boot.c:163`):

```c
BOOT_CODE static pptr_t alloc_rootserver_obj(word_t size_bits, word_t n) {
    memzero((void *)allocated, n * BIT(size_bits));  // C 版逐字节清零
    return allocated;
}
```

**Rust 版修复:** 分配后逐块 `clear_memory`，对齐 C 版 `memzero`：

```rust
for i in 0..n {
    clear_memory((allocated + i * bit!(size_bits)) as *mut u8, size_bits);
}
```

---

## Commit 4: `35658b3` — fix page table attribute

**文件:** `sel4_vspace/src/arch/aarch64/boot.rs` (+2, -2)

**症状:** roottask data abort: `esr=0x92000061` (DFSC=0x21=Alignment fault),
`far`=8 字节对齐但非 16 字节对齐，反汇编 `stp q0, q0, [x0]`。

**根因:** `map_it_frame_cap` 硬编码 `attr=0` → `DEVICE_nGnRnE`：

```rust
// 修复前: 所有用户帧被映射为 Device
let (ng, attr) = (1, 0);   // MAIR[0] = DEVICE_nGnRnE
```

**这一个 bug 同时解释多种现象:**
1. Device 对齐访问正常、非对齐无条件报错 → 代码能跑但 SIMD 写崩
2. Device = 非缓存 → bootinfo 读到旧值
3. QEMU 不建模 → QEMU 正常

**修复:**

```rust
let (ng, attr) = (1, mair_types::NORMAL as usize);   // MAIR[4] = NORMAL
```

**诊断:** Alignment fault + 代码能执行 + `SCTLR_EL1.A=0`
⇒ 该页被误映射为 Device，优先核对 PTE 的 MAIR 字段。

---

## Commit 5: `a23fd2c` — fix cache flush bug

**文件:** `kernel/src/syscall/invocation/decode/arch/aarch64.rs` (+16, -13)

### 修复 `decode_vspace_root_invocation` 中的 6 个 bug

用户空间调用 ARMVSpace Clean/Invalidate/Unify 操作时，
内核需要定位用户帧对应的物理地址并进行 cache 维护。
原实现存在多处错误，导致 cache flush 操作完全无效。

**Bug 1 — ASID 获取对象错误:**

代码在 `cap_vspace_cap` 上调用了 `cap_asid_pool_cap` 的 getter：
```rust
// 错误: vspace cap 上拿到的 asid 不对
let asid = cap::cap_asid_pool_cap(&cte.capability).get_capASIDBase() as usize;
```
```rust
// 修复: vspace cap 持有 capVSMappedASID
let asid = cap::cap_vspace_cap(&cte.capability).get_capVSMappedASID() as usize;
```

**Bug 2 — `lookup_pt_slot` 调用方式错误:**
```rust
// 错误: 把 vspace_root 裸指针当 PTE 用
let resolve_ret = ptr_to_mut(vspace_root).lookup_pt_slot(vptr!(start));
```
```rust
// 修复: 先构造 PTE 对象再调用 lookup
let mut root_pte = PTE::new_from_pte(vspace_root as usize);
let resolve_ret = root_pte.lookup_pt_slot(vptr!(start));
```
`vspace_root` 是 `*mut PTE` 指向页表顶层，但 `lookup_pt_slot` 需要 `PTE` 类型
（内含实际 PTE 值）。裸指针解引用得到的 `PTE` 是 0 值，导致查找从空页表
开始立即返回。

**Bug 3 — flush 范围 `end` 边界 off-by-one:**
```rust
// 错误: 把 [start, end] 当范围传给 flush
return decode_vspace_flush_invocation(label, ..., vptr!(end), ...);
// flush 内部用 (start < end) 判断，传入绝对 end 导致多 flush 一字节
```
seL4 的 cache flush syscall 约定 `end` 为开区间 `[start, end)`，
传入 `end - 1` 方为闭区间。
```rust
// 修复
return decode_vspace_flush_invocation(label, ..., vptr!(end - 1), ...);
```

**Bug 4 — `ptBitsLeft` 语义错误:**
```rust
// 错误: pageBitsForSize 的参数应该是 page_size 枚举值 (如 12=4K, 21=2M)
// ptBitsLeft 已经是地址剩余位宽 (如 12, 21)，不应该再传进 pageBitsForSize
let page_base_start = start & !mask_bits!(pageBitsForSize(resolve_ret.ptBitsLeft));
```
`ptBitsLeft`（如 12、21）本身就可以直接作为 `mask_bits!` 参数。
```rust
// 修复
let page_base_start = start & !mask_bits!(resolve_ret.ptBitsLeft);
```

**Bug 5 — PTE 类型检查过窄:**
```rust
// 错误: 只检查 pte_page（2MB 大页），跳过了 pte_4k_page（4KB 小页）
if ptr_to_ref(pte).get_type() != (pte_tag_t::pte_page) as usize {
    get_currenct_thread().set_state(ThreadState::ThreadStateRestart);
    return exception_t::EXCEPTION_NONE;
}
```
这意味着**只有 2MB 大页能被 cache flush，4KB 页直接静默跳过**。
```rust
// 修复: 使用 pte_is_page_type() 同时匹配 4K 和 2M
if !ptr_to_ref(pte).pte_is_page_type() {
    ...
}
```

**Bug 6 — `decode_page_clean_invocation` 变量未使用:**
```rust
// 错误: _vaddr 被标记为忽略，do_flush 参数直接用裸 start/end 没有加上 vaddr 偏移
let _vaddr = ...;
do_flush(label, start, end, pstart);
```
frame cap 的 `capFMappedAddress` 是该帧的**基地址**，用户传入的 flush 范围
是帧内偏移，物理起始地址应为 `vaddr + start`。
```rust
// 修复
let vaddr = ...;
do_flush(label, vaddr + start, vaddr + end - 1, pstart);
```

---

## Commit 6: `ff07a8e` — fix page map unmap bug

**文件:** `sel4_vspace/src/arch/aarch64/interface.rs` (+9, -9),
`kernel/src/syscall/invocation/invoke_mmu_op.rs` (+9, -3),
`sel4_vspace/src/arch/aarch64/machine.rs` (+31, -7)

### Bug 6a: `set_vm_root` 未将 ASID 写入 TTBR0_EL1 — TLB 刷新失效

**`sel4_vspace/src/arch/aarch64/interface.rs:106-109`**

**症状:** `invalidate_tlb_by_asid(asid)` 执行 `tlbi aside1` 无法刷掉对应 ASID
的 TLB 条目，unmapped 页仍可被访问。循环 flush ASID 0-16 可以，因为命中了
实际 ASID=0。

**根因:** 切换到用户页表时直接向 TTBR0_EL1 写入裸物理地址，ASID 字段为 0：

```rust
// 修复前: ASID 始终为 0
set_current_user_vspace_root(
    pptr!(thread_root_vspace.get_capVSBasePtr()).to_paddr().raw(),
);
```

**后果链:**
1. TTBR0_EL1 中 ASID=0
2. 硬件以 ASID=0 创建 TLB 条目
3. `invalidate_tlb_by_asid(1)` → `tlbi aside1, #(1<<48)` → 但 TLB 条目是 ASID=0
4. 刷不到

**修复:** 使用 `ttbr_new` 将 ASID 编码到 TTBR bits[63:48]：

```rust
set_current_user_vspace_root(ttbr_new(
    asid,
    pptr!(thread_root_vspace.get_capVSBasePtr()).to_paddr(),
));
```

> 对比原版 seL4: `vspace.c` → `armv_contextSwitch` → `setCurrentUserVSpaceRoot(ttbr_new(asid, pptr_to_paddr(vspace)))`

### Bug 6b: `unmap_page` / `unmap_page_table` ASID 级 flush → VA 级 flush

**`sel4_vspace/src/arch/aarch64/interface.rs:203, 238`**

原版 seL4 在 unmapping 时使用 `invalidateTLBByASIDVA(asid, vptr)`
（`tlbi vae1`，只刷一个 VA）。Rust 版错误地使用了
`invalidate_tlb_by_asid(asid)`（`tlbi aside1`，刷整个 ASID）。

```rust
// 修复前
invalidate_tlb_by_asid(asid);
// 修复后
invalidate_tlb_by_asid_va(asid, vaddr);  // unmap_page_table
invalidate_tlb_by_asid_va(asid, vptr);   // unmap_page
```

精确 VA flush 的关键在于 `(asid << 48) | (vaddr >> PAGE_BITS)` 的编码，
实现中提取为 `mva_plus_asid` 变量，同时用于 local 和 remote TLB invalidate。

### Bug 6c: `invoke_page_map` 条件 flush

**`kernel/src/syscall/invocation/invoke_mmu_op.rs:170, 177-179`**

- 恢复 `tlbflush_required` 判断：仅当旧 PTE 已有映射时才 flush（`pt_slot.get_type() != pte_invalid`）
- import 从 `invalidate_tlb_by_asid` 改为 `invalidate_tlb_by_asid_va`

### Bug 6d: `invoke_page_unmap` write_volatile

**`kernel/src/syscall/invocation/invoke_mmu_op.rs:119-126`**

unmap 后 cap 字段清零通过 `write_volatile` 确保编译器不重排/消除：
```rust
let mut raw = frame_slot.capability.clone();
unsafe {
    let fp = &mut raw as *mut cap as *mut cap_frame_cap;
    (*fp).set_capFMappedAddress(0);
    (*fp).set_capFMappedASID(ASID_INVALID as u64);
    core::ptr::write_volatile(&mut frame_slot.capability, raw);
}
```

### Bug 6e: `machine.rs` cache 操作修复

**`sel4_vspace/src/arch/aarch64/machine.rs`** — 7 项修复:

1. **`invalidate_local_tlb_va_asid` 重新实现:**
   DSB/TLBI/DSB/ISB 屏障序列原为分散调用，现整合为一条内联 asm，确保
   `tlbi vae1` 在 DSB 之间正确执行。

2. **`clean_by_va_pou` 指令修正:**
   `dc cvau`（clean to point of unification）→ `dc civac`（clean+invalidate to point of coherency）+ 后接 `dsb`（原为 `dmb`）。
   确保内核修改的页表数据对用户态（不同 ASID 空间）可见。

3. **所有 cache range 函数增加空范围保护:**
   `clean_cache_range_ram`、`invalidate_cache_range_i`、`clean_cache_range_poc`、
   `clean_cache_range_pou`、`clean_invalidate_cache_range_ram`、
   `invalidate_cache_range_ram` 全部添加 `if end <= start { return; }`，
   防止 `for idx in LINE_INDEX(start)..LINE_INDEX(end)+1` 在 `start>end` 时 panic。

---

## 验证

修复后 sel4test 单核测试套件全部通过。RPi4 平台上内核可正常启动，中断正常触发，

---

# Hypervisor 移植记录

## 概述

在非 hypervisor RPi4 移植的基础上，进一步支持 `CONFIG_ARM_HYPERVISOR_SUPPORT`
（内核运行在 EL2，用户态通过 VTTBR_EL2 的 stage-2 页表管理），并同时通过
RPi4 与 QEMU arm-virt（a72）的 sel4test。

改动起点 commit：`89ffda4c`（"reL4 kernel loader success"），主要 commit：

| Commit | 描述 |
|--------|------|
| `e0e67ad` | fix some hypervisor features |
| `dfabdae` | fix pt test bug on qemu |
| `6d4edec` | add all hypervisor features in kernel and pass sel4test |
| `a6a1e41` | work save for multi-feature support |
| `431b19f` | modify build config system |
| `8281bf5` | support hypervisor and pass sel4test on rpi4 and qemu |

## 关键改动

### 1. HCR_EL2：补齐 TWI/TWE trap

**文件:** `kernel/src/arch/aarch64/vcpu.rs`

hypervisor 下 native 线程跑在 EL0，异常路由到 EL2（TGE=1）。C 版 `HCR_COMMON`
（`DISABLE_WFI_WFE_TRAPS=false`）包含 TWI/TWE，Rust 版之前漏掉，导致 EL0 的
WFI/WFE 走 EL1 VBAR 而不是直接 trap 到 EL2。

```rust
const HCR_NATIVE: u64 = HCR_COMMON
    | HCR_EL2::TGE::EnableTrapGeneralExceptionsToEl2.value
    | HCR_EL2::SWIO::SET.value
    | bit!(12)               // DC
    | bits!(26, 25, 21)      // TVM | TTLB | TAC
    | bits!(14, 13);         // TWE | TWI  ← 补齐
```

最终 `HCR_NATIVE = 0x8E28703B`，与 C 版一致。

### 2. VTCR_EL2：RES1 + PA 宽度选择

**文件:** `kernel/src/arch/aarch64/vcpu.rs`

- 补上 VTCR_EL2 bit31（RES1）。
- 按 PA 宽度选择 stage-2 起始级别：
  - 40-bit（feature `pa_40bit`）：T0SZ=24, SL0=1（3-level stage-2）
  - 44-bit（默认）：T0SZ=20, SL0=2（4-level stage-2）

### 3. MAIR_EL2：8×8 / 16×4 双布局兼容

**文件:** `kernel/src/arch/aarch64/vcpu.rs`

MAIR_EL2 被两种翻译机制**共用但解释不同**：

| 使用者 | 布局 |
|--------|------|
| 内核窗口（EL2 stage-1, TTBR0_EL2） | 8×8-bit（Attr0-7） |
| 用户态（stage-2, VTTBR_EL2） | 16×4-bit（Attr0-15） |

elfloader 只按 stage-1 的 8-bit 布局设置，导致 stage-2 的 Attr15=Device，
Normal 页在真机上 fault。内核在 `vcpu_boot_init` 覆盖为双布局兼容值：

```rust
const MAIR_EL2_VALUE: u64 = (0x00u64 << 0)   // Attr0: Device-nGnRnE
    | (0x04u64 << 8)                          // Attr1: Device-nGnRE
    | (0x0cu64 << 16)                         // Attr2: Device-GRE
    | (0x44u64 << 24)                         // Attr3: Normal-NC
    | (0xffu64 << 32)                         // Attr4: Normal WB-WA（内核窗口 NORMAL=4 引用）
    | (0xaau64 << 40)                         // Attr5: Normal-WT
    | (0xf0u64 << 56);                        // Attr7 高半=0xf → stage-2 Attr15=Normal
// = 0xF000_AAFF_440C_0400
```

### 4. stage-2 页表属性：S2_NORMAL + 4-bit AttrIndx

**文件:** `sel4_vspace/src/arch/aarch64/machine.rs`、`pte.rs`、`boot.rs`

- `mair_types` 新增 `S2_NORMAL = 15`（stage-2 Normal WB-WA）。
- `pte_new_page` / `pte_new_4k_page` 的 AttrIndx 用 `& 0xF`（stage-2 是 4 位）。
- `make_user_pte` / `map_it_frame_cap` 在 hypervisor 下 cacheable 帧用 `S2_NORMAL`，
  device 帧用 `DEVICE_nGnRnE`。

### 5. ⭐ stage-2 TLB 刷新（核心 bug）

**文件:** `sel4_vspace/src/arch/aarch64/machine.rs`、`interface.rs`

**症状:** map/unmap 后旧映射仍生效（FRAMEEXPORTS0001 / map-unmap 失败）。

**根因:** Rust 版在 hypervisor 下用 `tlbi vae1` / `tlbi aside1`，这两个指令
只刷 **EL1 stage-1** TLB，刷不到 stage-2。注释里"在 EL2 执行时 ASID 被当作
VMID"是错误的。

**正确做法（对齐 C 版 `armv/tlb.h`）:**
- `tlbi ipas2e1`：按 IPA 刷 stage-2（当前 VMID）。
- `tlbi vmalls12e1`：刷 stage-1 + stage-2（当前 VMID）。

这两个指令只作用于 **VTTBR_EL2 当前 VMID**，必须先切 VMID → 刷 → 恢复：

```rust
// invalidate_local_tlb_ipa_vmid
let vttbr = VTTBR_EL2.get();
let v = (vttbr >> 48) & 0xffff;
let vmid = (ipa_plus_vmid >> 48) & 0xffff;
let ipa = ipa_plus_vmid & 0xfffffffff;   // C: "[0:35] bits are IPA"
if v != vmid { VTTBR_EL2.set((vmid as u64) << 48); dsb(); isb(); }
asm!("tlbi ipas2e1, {}", in(reg) ipa);
dsb(); asm!("tlbi vmalle1"); dsb(); isb();
if v != vmid { VTTBR_EL2.set(vttbr); dsb(); isb(); }
```

`invalidate_tlb_by_asid_va` / `invalidate_tlb_by_asid` 的 hypervisor 分支
分别改用 `invalidate_local_tlb_ipa_vmid` / `invalidate_local_tlb_vmid`。

### 6. 页表 cache clean：dc cvau（PoU）

**文件:** `sel4_vspace/src/arch/aarch64/machine.rs`

页表写入后的 clean 用 `dc cvau`（clean to PoU）+ dmb，对齐 C 版
`cleanByVA_PoU`（不区分 stage-1/stage-2，统一 PoU）。

### 7. instruction cache：VIPT 整个 invalidate

**文件:** `sel4_vspace/src/arch/aarch64/machine.rs`

A72 L1 I-cache 是 VIPT，hypervisor 下 flush 用的是内核别名地址，无法正确
索引 VIPT I-cache。`invalidate_cache_range_i` 在 hypervisor 下改为整个
I-cache invalidate（`ic iallu`），对齐 C 版 `invalidateCacheRange_I`
（`ICACHE_VIPT + hypervisor` 分支）。

### 8. do_flush：地址转换移到调用处

**文件:** `sel4_vspace/src/arch/aarch64/interface.rs`、`decode/arch/aarch64.rs`

对齐 C 版结构：`do_flush` 只负责 flush 给定地址；hypervisor 的内核 VA 计算
（`paddr_to_pptr`）放在调用处 `decode_page_flush` / `decode_vspace_flush_invocation`。

### 9. pa_40bit feature

**文件:** `Cargo.toml`、`sel4_common`、`sel4_vspace`

40-bit PA（QEMU）→ 3-level stage-2，44-bit PA（RPi4）→ 4-level stage-2，
通过 `pa_40bit` feature 门控 `PT_LEVELS` / `UPT_LEVELS` / `VSPACE_INDEX_BITS` /
`SEL4_VSPACE_BITS` / `SEL4_USER_TOP` 等。

### 10. YAML 配置拆分

**文件:** `rel4_config/`

拆分为 `platform/`（cpu/timer/device/memory）和 `definitions/`（build 配置），
新增 `qemu-arm-virt_hyp.yml`、`bcm2711_hyp.yml`；`rel4_config` 支持
`get_platform_yaml_path(platform, hypervisor)` 选择 `_hyp` 变体。

### 11. 平台相关

- `KERNEL_TIMER_IRQ`：hypervisor=26（hyp timer），非 hypervisor=27（virtual timer）。
- `gicv2_vcpu_ctrl`（GICH）仅在 hypervisor 模式启用。
- `vmem_offset` 匹配 `PPTR_BASE`（hypervisor `0x8000000000`，非 hypervisor `0xffffff8000000000`）。
- GIC VCPU 接口地址统一 `KDEV_BASE + 0x3000`。

## 验证

hypervisor 模式 sel4test 在 RPi4 与 QEMU arm-virt（a72）均通过，包括
FRAMEEXPORTS0001、map/unmap、CACHEFLUSH 系列。