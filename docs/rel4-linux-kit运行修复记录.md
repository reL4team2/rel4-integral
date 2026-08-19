# rel4-linux-kit 运行修复记录

本文记录让 rel4-linux-kit 在 rel4 内核上成功启动所修复的问题，以及一个待调查的 sel4test release 模式卡死问题。

---

## 问题一：invocation label 不匹配导致 `FailedLookup`

### 现象

rel4-linux-kit 的 root-task 启动时，在 `root-task/src/cspace.rs:36` 的
`LeafSlot::new(0).delete().unwrap()` 处 panic：

```
panicked at root-task/src/cspace.rs:36:31:
called `Result::unwrap()` on an `Err` value: FailedLookup
```

内核日志（诊断期间加了临时打印）显示：

```
FailedLookup: DepthMismatch bitsFound=64 bitsLeft=52
```

### 根因

内核的 `MessageLabel` 枚举（`sel4_common/src/arch/{aarch64,riscv64}/message_info.rs`）
比 libsel4 XML（标签的 ABI 真相）**多了三个标签**：

- `TCBSetFlags`
- `DomainScheduleConfigure`
- `DomainScheduleSetStart`

这三个标签在 libsel4 的 XML 里根本不存在。多出来的 `TCBSetFlags` 使从
`CNodeRevoke` 开始的所有标签整体 +1（`DomainSchedule*` 又让 ARM 标签再 +2）。

于是用户态（rust-sel4 fork 的 `invocation_label::CNodeMint = 20`）发出的 mint，
被内核按 `20` 匹配成了 **`CNodeCopy`**（内核里 `CNodeCopy = 20`）：

- mint 被当成 copy 处理，**忽略 badge（arg5）**；
- `slot::CNODE` 被拷成源 cnode（`guardSize=0`），mint 的 guard（`skip_high_bits(24)=40`）被丢弃；
- 下一步 delete 用 depth=64 去解析这个 `guardSize=0` 的 cnode，第一层只吃掉 12 位、
  又撞上一个 `guardSize=52` 的 cnode（64 位）→ `DepthMismatch(64, 52)`。

这三个多余标签是 commit `0ec1795`（"try run vm, work save"）引入的；在此之前的
commit `6d4edec`（"add all hypervisor features in kernel and pass sel4test"）标签是正确的，
所以当时 sel4test 能通过。

### 修复

从 `MessageLabel` 枚举中删除这三个多余标签（aarch64、riscv64 两个文件都改）：

- `sel4_common/src/arch/aarch64/message_info.rs`
- `sel4_common/src/arch/riscv64/message_info.rs`

删除 `TCBSetFlags`、`DomainScheduleConfigure`、`DomainScheduleSetStart` 后，
内核标签重新与 libsel4 XML 对齐（`CNodeMint = 20` 等）。

---

## 问题二（顺带修复）：lookup fault 信息未传递

### 现象

内核解析 CNode 失败时，`current_lookup_fault` 一直是残留值，用户态拿到的
`seL4_FailedLookup` 里没有具体的 fault 类型（`InvalidRoot` / `GuardMismatch` /
`DepthMismatch`），难以定位问题。

### 根因

`resolve_address_bits` 位于 `sel4_cspace` crate，访问不到内核全局变量
`current_lookup_fault`，因此三个失败路径都没有设置 fault（C 参考实现是设置的）。

### 修复

- `sel4_cspace/src/structures.rs`：`resolveAddressBits_ret_t` 增加 `fault: lookup_fault` 字段；
- `sel4_cspace/src/cte.rs`：`resolve_address_bits` 在 invalid root / guard mismatch /
  depth mismatch 三个失败路径上设置 `ret.fault`；同时把 `n_bits - guardBits` 改成
  `n_bits.wrapping_sub(guardBits)` 避免下溢；
- `kernel/src/syscall/utils.rs`：`lookup_slot_for_cnode_op` 里 `current_lookup_fault = res_ret.fault`。

---

## 待调查问题：sel4test release 模式卡死

### 现象

内核不变（release 构建），仅 sel4test（C 用户态）使用 release 模式编译时卡死
（最终进入 `idle_thread`），debug 模式正常。

### 可能原因（尚未定位）

- `-DNDEBUG` 把 `assert` 编译掉；
- 自旋等待的变量缺 `volatile`，release 下被编译器缓存进寄存器导致忙等不退出；
- 未初始化变量（debug 下碰巧为零、release 下是垃圾值）；
- 写入 IPC buffer / 共享内存的操作被编译器优化掉。

### 待做

- 确认 sel4test release 的具体 CFLAGS（`-O2`/`-O3`、是否有 `NDEBUG`）；
- 用 GDB 定位卡住位置（`bt`），确定是哪个测试、哪个函数；
- 二分优化级别（先关 `NDEBUG`，再试 `-O1`），缩小范围。

---

## 遗留提醒

用户态 rust-sel4 fork 的 libsel4 里**没有 VCPU 标签**（`ARMVCPUSetTCB` 等），而内核在
hypervisor 下含这些标签。当前 root-task 启动不受影响（只用通用 IRQ 标签），但后续
若要跑 Linux 虚拟机（需要 VCPU / ARM IRQ trigger 操作），仍需对齐这一部分标签。

---

## 方案 A + B：自动化标签生成与一致性检查

上面的“手动删除标签”只是临时修复。为避免以后各种配置（mcs / smp / hypervisor / smc）
再次发生标签漂移，已改为**从 libsel4 XML 自动生成 `MessageLabel`**（方案 A），并加
**编译期一致性检查**（方案 B）。

### 方案 A：从 XML 生成

- 新增 `rel4_config/src/message_label_gen.rs`：解析 libsel4 的三个 XML 接口文件
  （`object-api.xml` → `object-api-sel4-arch.xml` → `object-api-arch.xml`，顺序与
  libsel4 一致），生成 `MessageLabel` 枚举。
- 配置项 → Rust feature 映射：
  `CONFIG_KERNEL_MCS→kernel_mcs`、`CONFIG_ENABLE_SMP_SUPPORT→enable_smp`、
  `CONFIG_ARM_HYPERVISOR_SUPPORT→hypervisor`、`CONFIG_ENABLE_SMC→enable_smc`；
  其余（`CONFIG_TK1_SMMU`、`CONFIG_HARDWARE_DEBUG_API`、`CONFIG_ARM_SMMU` 等）rel4
  不支持 → 恒为 false，直接省略对应标签。
- `sel4_common/build.rs` 在构建时定位 libsel4 目录（可用 `LIBSEL4_DIR` 覆盖），
  调用生成器把枚举写到 `OUT_DIR/message_label.rs`。
- `sel4_common/src/arch/{aarch64,riscv64}/message_info.rs` 改为
  `include!(concat!(env!("OUT_DIR"), "/message_label.rs"));`，不再手写枚举。

### 方案 B：编译期一致性检查

生成器在枚举下方输出 `const _: () = assert!(MessageLabel::X as usize == N);` 逐项校验
每个标签的判别值，并校验 `nArchInvocationLabels` 的总数。若生成器与编译器对标签顺序/
cfg 映射的理解不一致，构建直接失败。

### 验证

- 非 hypervisor（`--features have_fpu`）：`CNodeMint=20`、`ARMPageClean_Data=39`、
  `ARMIRQIssueIRQHandlerTrigger=46`、总数 47，编译通过。
- hypervisor（`--features have_fpu,hypervisor`）：VCPU 标签 46-50、
  `ARMIRQIssueIRQHandlerTrigger=51`、总数 52，编译通过。

### 涉及文件

- `rel4_config/Cargo.toml`（新增 `roxmltree`）
- `rel4_config/src/lib.rs`
- `rel4_config/src/message_label_gen.rs`（新增）
- `sel4_common/build.rs`
- `sel4_common/src/arch/aarch64/message_info.rs`
- `sel4_common/src/arch/riscv64/message_info.rs`
