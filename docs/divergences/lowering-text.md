# 分歧登记 — 子系统线 D（sla-lowering / sla-codegen-text）

参照源：Zig `src/lowering_rules.zig`（9,705 行）、`src/codegen.zig`（16,288 行）
及 `src/control_flow_rules.zig`，本机只读仓 `~/workspace/local-sa/sa_plugin_sla/`。
分类：(a) 与 Zig 行为/产出分歧 · (b) Rust 纪律逼出的表达受阻点 · (c) Zig 源码可疑底层问题。
原则：分歧先登记再分类，不许为对拍绿静默改形。Rust 侧只镜像 + 登记，不改 Zig 行为迁就。

## 协调线开放项的裁决（D-C4 / D-C5）

### D-T1 (a) D-C4 结案：导入列表是源导入的纯函数
- 协调线疑问：golden 的导入行数（01 为 8 行、其余 7 行）疑似与 test 体可达性有关。
- 实证：发射导入 = 程序（展开后）导入声明按序 + 5 条固定导入
  （string.sa、string_format.sa、io/print.sai、fmt.sai、core/panic.sa）。
  01 比其余 fixture 多一行，仅因其源码有第 3 条导入（`sa_std/string.sa`）；
  02/03/04/07 均为 2 条源导入 → 7 行。与 test 体无关，假设不成立。
- Rust：`program.rs` 按此模型实现，fixture 01 已在场地逐字节通过。

### D-T2 (a) D-C5 结案：07 中两个不可见计数器消耗点
- `tmp_7`：codegen.zig:15871 —— 无结果槽的 if（void if）在 `genExpr` 末尾
  `return try self.newTmp()`，分配一个从不发射的临时寄存器作为丢弃结果。
  fact 的 if 双分支均 return、无值，add 因此从 tmp_8 起号。
- `L_MERGE_2`：codegen.zig 约 15744 —— 普通 if 路径无条件 `newLabel("L_MERGE")`，
  仅当 `needs_merge` 为真时才发射标签行。fact 的 if 双分支终止且有 else，
  `needs_merge=false`，标签被分配但不出现；test 入口因此是 L_TEST_ENTRY_3。
- Rust：`expr_if.rs` 两点均逐字镜像（含丢弃 tmp 的分配）。

## (c) Zig 源码可疑底层问题

### D-T3 (c) 调用实参清理里硬编码函数名 `sum` ★ 静默兜底族
- Zig 位置：`codegen.zig:9830-9832`（`emitLoweredCallArgCleanups`）。
  `if (skip_release_name == "sum")` 时**整段跳过**该调用的全部实参释放，
  无注释、无计划函数支撑。对名为 sum 的用户函数同样生效（名字碰撞即改变语义）。
- Rust：逐字镜像（`call.rs`），待主线裁决是否属缺陷。

### D-T4 (c) 未知被调用名静默假定为宏 EXPAND
- Zig 位置：`codegen.zig` 约 15540（`genExpr` call 路径末尾兜底）。
  函数表/外部表/内建分发全部未命中时，不报错，直接发射
  `EXPAND {name} {result}, {args...}`。拼写错误的函数名会静默变成宏展开，
  错误推迟到 SA 汇编层甚至链接层才暴露（或永不暴露）。
- Rust：逐字镜像（`call.rs` 末尾兜底臂），待主线裁决。

### D-T5 (c) println 对未知类型实参只打印 1 字节
- Zig 位置：`codegen.zig` 约 2397-2417（`emitPrintln` 末尾兜底臂）。
  `{}` 孔实参类型不在 int/float/bool/String/借用等已知形态内（或检查器
  未给类型）时，发射 `call @sa_print_bytes(&{ptr}, 1)` —— 恰好打印 1 字节，
  无诊断。格式化输出静默截断为单字节。
- Rust：逐字镜像（`println.rs` 兜底臂），待主线裁决。

### D-T6 (c) println 长度口径不一致：段用原始跨度、字符串实参用转义后长度
- Zig 位置：`codegen.zig` 约 2185（字面段以 `i - start` 原始跨度为长度）
  vs 约 2195（字符串实参以 `escapedStringByteLen` 为长度）。
  格式串段内含转义序列时，两条路径对同一常量的长度计算口径不同，
  段路径会把转义文本按原始字节数打印（多打字节）。
- Rust：逐字镜像两条口径（`println.rs`），待主线裁决。

### D-T7 (c) 构建产物中的 @test 空壳来自上游裁剪，非 codegen 行为
- `genTestDecl`（codegen.zig:7610）忠实生成所给函数体；golden 里 @test
  体为空，是流水线在 codegen 之前（plugin_compile 构建模式）把 test 体
  清空所致。后果：构建产物的 @test 只是空壳，测试覆盖在产物层不可见，
  「产物即已测」的读法会高估实际执行过的断言数。
- Rust：`gen_test_decl` 同样忠实生成所给体；对拍台以空体喂入以匹配
  build 模式 golden。此为流水线口径问题，登记备主线裁决。

### D-T8 (c) shouldAutoBorrowReceiverArg 冗余死分支（轻微）
- Zig 位置：`lowering_rules.zig:6221-6226`。内层
  `if (arg_ty is borrow) return true;` 之后无条件 `return true`，
  内层判定对结果无影响，疑似重构残留。
- Rust：原样镜像（`call_args.rs`），行为等价，仅登记。

### D-T9 (c) genStmt 对未处理语句种类静默无输出
- Zig 位置：`codegen.zig:10650` 的 `genStmt` switch 以 `else => {}` 收尾：
  未覆盖的语句种类不产出任何文本、也不报错，程序会静默缺失整段语句。
- Rust：**有意不迁就** —— 未覆盖语句种类响亮报错（见 D-T12）。

## (b) Rust 纪律逼出的重构点

### D-T10 (b) 检查器表：指针键映射 → CheckInfo trait 边界 ★ 结构性
- Zig：codegen 直接读 `tc.expr_types / cleanups / phi_cleanups` 等以
  `*Node` 为键的表。Rust 的 owned 树没有稳定指针网（节点在 Vec/Box 中
  移动），该隐式契约被显式化为 `sla-lowering::CheckInfo` trait，
  键 = `node_key(node)`（节点当前地址，与 checker 线的
  `node as *const Node as usize` 公式逐字一致）。
- 推论（对拍台实证）：任何对 AST 节点的 clone 都会使地址键失效——
  println 实参曾因一处 clone 丢失类型事实、落入 1 字节兜底臂（D-T5），
  已改为全程借用原节点。接线方（parser/checker 线）须保证喂入 codegen
  的树与登记事实时是同一棵树、且其后不再搬动节点。

### D-T11 (b) hoisted_allocs 由参数穿线改为发射器状态
- Zig：`genExpr/genStmt` 把 `hoisted_allocs` 列表逐层作参数传递。
- Rust：`Codegen.hoisted_allocs` 字段在函数入口由 collect 预填。
  产出不可见，仅结构差异。

### D-T12 (b) 未覆盖路径一律响亮报错（与 D-T9 对应）
- 本线所有未移植的发射域（for 语句、switch/match、if-let/while-let、
  结构体/枚举/元组/数组字面量、字段/下标/切片表达式 (部分)、宏族、
  async/future 运行时、dyn 分发等，完整清单见 `sla-lowering/src/deferred.rs`
  的 DEFERRED_FAMILIES 登记表）在 Rust 侧返回带 Zig 行号参照的
  `Unsupported` 错误，不产出部分文本。这是与 Zig 的有意行为分歧：
  对拍口径下凡触发即红，不会静默缺失。

## (a) 产出分歧与未定归因

### D-T13 (a) 浮点常量格式化未验证（风险登记）
- Zig `emitFloatConst` 用 `{d}` 打印 f64；Rust 用 `Display`（最短回环）。
  非整值浮点的十进制拼写可能不同。切片内无浮点，未触发；
  后续含浮点 fixture 落地时须专项对拍。

### D-T14 (a) fixture 04 出口清理的归因不可分辨（待 checker 线边界转储裁决）
- golden 中 L_WHILE_EXIT 处的 `!i` / `!limit` 既可归因于 while 语句的
  genStmt 尾随清理表，也可归因于紧随其后的 return 语句清理表 —— 字节
  完全相同、不可分辨。本线对拍台把 [i, limit] 挂在 return 语句上。
  真实检查器的挂载点由 checker 线在其边界转储落地后对账；
  若归因不同，仅测试台事实登记需调整，发射代码不变。

## 已核对、非分歧（防重复登记）

- `planRefCellCallArgLifecycle`（lowering_rules.zig:3631-3635）：
  keep / release_borrow_handle / release_value 三态，`shouldRelease`
  对两个 release 变体均为真。本线初版近似（借用句柄不释放）已按原文
  逐字修正，`call_args.rs` 与 Zig 一致。
- `move_expr`（codegen.zig:13063）：结果寄存器带 `^` 前缀
  （`^{inner}`），已逐字核对并镜像于 `expr.rs`。

## 对拍验证状态（2026-11-07）

- 场地（Kaggle 2201）构建：sla-lowering、sla-codegen-text 均通过；
  sla-lowering 单元测试 9/9 通过。
- 逐字节对拍：fixture 01 已在场地验证全等。02/03/04/07 的两处根因
  （D-T10 的 clone 失效、字面量/标识符类型未登记致实参误判所有权转移）
  已定位并修复，**复验待新场地**（原场地 hold 于约 01:15 UTC 到期关闭，
  修复轮未及上场）。修复后预期 5/5，结论以场地复跑为准，不提前报绿。
