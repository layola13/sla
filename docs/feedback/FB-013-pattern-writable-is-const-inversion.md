# FB-013 — definePatternBindings 的 `writable` 参数被直塞进 `is_const` 槽位（语义反转）

- **反馈环批次**：第二批（checker 二期扩围线，2026-11-07）
- **源登记**：`docs/divergences/checker.md` D-C15（checker 线）
- **分类**：Zig 侧缺陷候选 —— 源码直读确认（命名与语义反转，行为后果见下）
- **Rust 侧处置**：只镜像 + 登记。Rust `define_pattern_bindings`
  按 Zig 原样把该参数写入 `is_const`（`crates/sla-checker/src/check_pattern.rs`
  模块头有明示），不做「纠正」。

## 机制（Zig 源码位置，本机参照树行号）

- `type_checker.zig:1649` `definePatternBindings(scope, pattern, ty, ctx, writable)`：
  函数体内把 `writable` 直接作为 `defineSymbol(..., is_const)` 的实参传入。
  参数名说「可写」，槽位语义是「常量」——两者恰好反转。
- 各调用点传值（字面量）：if-let 链（:5252 臂）传 `true`、while-let 传
  `true`、match 臂传 `true`、let-else（:2858 臂）传 `false`。
- 后果：if-let / while-let / match 的模式绑定实际是 **const** 绑定
  （不可再赋值），let-else 的模式绑定实际是**可变**绑定——与各调用点
  按参数名阅读时的直觉完全相反。若某调用点作者按名义语义改传值，
  行为将静默翻转。

## 建议修复形态（仅建议，不预判主线最终形态）

二选一并全仓统一：(a) 参数改名为 `is_const` 并逐调用点核对传值与
预期绑定可变性一致；(b) 保持参数名 `writable`，函数体内传
`!writable`，同时逐调用点翻转现有传值。无论哪种，四个调用点的
目标可变性应先在 sala/设计层定明（模式绑定是否允许再赋值），
再动代码——本单只登记反转事实，不预判目标语义。

## Rust 侧单测锚点

if-let / while-let / let-else / match 的模式绑定测试组
（tests_phase2 / tests_phase2b）钉住现行行为；Zig 侧若翻转，
绑定可变性断言随对拍更新。
