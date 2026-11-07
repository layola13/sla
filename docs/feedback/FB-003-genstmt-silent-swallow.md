# FB-003 — genStmt 以 `else => {}` 静默吞未处理语句种类

- **反馈环批次**：第二批（lowering/text 线 (c) 类七条之首）
- **源登记**：`docs/divergences/lowering-text.md` D-T9；合编 `docs/DIVERGENCES.md`
- **分类**：Zig 侧缺陷候选 —— 源码机制定案（静默兜底族，与 SAB 吞错同科）
- **Rust 侧处置**：**有意不镜像**。Rust 发射器对未覆盖语句种类响亮报
  Unsupported（D-T12），不产出部分文本；不替 Zig 改行为。

## 机制（Zig 源码位置）

- `src/codegen.zig:10650`：`genStmt` 的语句 switch 以 `else => {}`
  收尾。任何未被显式覆盖的语句种类进入该臂后**不产生任何输出、
  也不产生任何诊断**——编译「成功」，产物里整段语句凭空消失。

## 影响

- 缺失是静默的：源程序与产物程序语义不一致且无任何信号，下游
  （SA 汇编、链接、运行）都不会报错，只会在行为层表现为「某段
  逻辑没执行」。
- 与主线既有判例同科：SAB 侧 `else |_| <字面量>` 静默吞错、
  「禁止静默兜底/降级」用户定规——文本 codegen 的总入口 switch
  兜底恰是最危险的一处，因为它吞的是整条语句而非单个值。

## 建议修复形态（仅建议，不预判主线最终形态）

`else` 臂改为响亮报错（返回 CompileError 并携带语句种类与位置），
或在覆盖审计后把 switch 改为穷尽式（新语句种类加入 AST 时编译期
即暴露未处理点）。修复时建议同步普查 codegen/lowering 内同形
`else => {}` 兜底臂。

## Rust 侧锚点

`crates/sla-lowering/src/deferred.rs` 的 DEFERRED_FAMILIES 登记表
列全 Rust 侧响亮拒绝的未移植发射域；genStmt 对应路径在
`crates/sla-codegen-text` 中无静默臂。修复后差分复核口径：
任一语句种类输入，两侧要么产出一致、要么 Zig 侧响亮报错。
