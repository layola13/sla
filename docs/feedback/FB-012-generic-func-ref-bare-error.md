# FB-012 — generic_func_ref 臂是无诊断的裸 CompileError

- **反馈环批次**：第二批（checker 二期扩围线，2026-11-07）
- **源登记**：`docs/divergences/checker.md` D-C14（checker 线）
- **分类**：Zig 侧缺陷候选 —— 源码直读确认（诊断质量问题）
- **Rust 侧处置**：只镜像 + 登记。Rust 镜像返回同种 `CompileError`
  并以包裹层回退文案为消息（`crates/sla-checker/src/check_expr.rs`）。

## 背景与最小输入

泛型实例化由 monomorphizer 在检查器之前完成（plugin_compile 流水线序：
monomorphize → type-check），故正常流水线里检查器不应见到
`generic_func_ref` 节点。一旦见到（单态化漏网、工具直调检查器、
未来前端直接构造 AST），Zig 的行为是：

```sla
fn id<T>(x: T) -> T { return x; }
fn g() -> int {
    let f = id<int>;   // 若此引用形态漏过单态化进入检查器
    return f(1);
}
```

## 机制（Zig 源码位置，本机参照树行号）

- `type_checker.zig:3209`：`generic_func_ref` 臂全文为
  `return TypeError.CompileError;`——不调用 `setError`，无函数名、
  无泛型实参、无位置信息。调用方包裹层只能合成
  "checkExpr failed at node tag generic_func_ref" 一类回退文案。
- 场地探针 `p_generic`（`id<int>(5)` 全流水线）rc=0，证实正常路径
  由单态化承接、本缺陷只影响漏网形态的可诊断性。

## 建议修复形态（仅建议，不预判主线最终形态）

在 :3209 返回前补 `setError`：注明泛型函数名与「实例化应由
monomorphizer 完成」的原因，使用户/工具侧能区分「单态化缺陷」
与「源码错误」。行为（拒绝）不变，仅补诊断。

## Rust 侧单测锚点

`generic_func_ref_rejected_compile_error`（tests_phase2b）钉住
拒绝行为与错误种别；Zig 补诊断后，Rust 侧消息文案随对拍更新。
