# FB-011 — switch 模式完全不检查：模式表达式不做类型检查、不绑名、无穷尽性

- **反馈环批次**：第二批（checker 二期扩围线，2026-11-07）
- **源登记**：`docs/divergences/checker.md` D-C13（checker 线）
- **分类**：Zig 侧缺陷候选 —— 源码直读确认（结构性缺失，非单点误判）
- **Rust 侧处置**：只镜像 + 登记。Rust `check_switch`
  （`crates/sla-checker/src/check_match.rs`）同样只检查 scrutinee
  与各臂体，模式原样跳过，与 Zig 一致。

## 最小输入（形态示意）

```sla
fn f(x: int) {
    switch x {
        case y + undefined_name => { return; }
        case "a string against int" => { return; }
    }
}
```

模式中的未定义名、与 scrutinee 类型不符的字面量均不会被 switch 臂报告；
穷尽性同样无人检查（无 default 且未覆盖全部值时静默通过，函数尾
类型检查靠 `exprTerminates` 的 default 存在性间接兜底）。

## 机制（Zig 源码位置，本机参照树行号）

- `type_checker.zig:5464` 起 `switch_expr` 臂：`_ = try self.checkExpr(scrutinee)`
  后逐 case 恢复状态、检查 case 体；`case.pattern` 在整个臂中从未被
  传入任何检查函数——无模式类型检查、无模式绑定定义、无穷尽性分析。
- 对照 `match_expr` 臂（:5494 起）对模式有完整的变体/绑定/元数检查，
  switch 与 match 的模式严谨度在同一文件内分叉。

## 建议修复形态（仅建议，不预判主线最终形态）

switch 臂对每个 case 模式至少做一次 `checkExpr`（字面量/标识符形态），
并对模式类型与 scrutinee 类型做 `typesEqual` 校验；穷尽性是否引入
（要求 default 或全覆盖）属语言语义决定，应单独立项裁定，不在本单预判。

## Rust 侧单测锚点

`switch_merge_consumes_single_case_binding`、
`switch_all_terminating_restores_pre_state`（tests_phase2）钉住 switch
臂的状态合并行为；模式检查若在 Zig 侧补齐，Rust 侧需同步补模式检查
并新增对拍测试。
