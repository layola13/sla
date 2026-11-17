# FB-010 — 解构消费站点无 Copy 豁免：Copy 元组变量解构后仍被消费

- **反馈环批次**：第二批（checker 二期扩围线，2026-11-07）
- **源登记**：`docs/divergences/checker.md` D-C12（checker 线）
- **分类**：Zig 侧缺陷候选 —— **已实证确认**（见下）
- **Rust 侧处置**：只镜像 + 登记。Rust 检查器在解构站点无条件消费根绑定
  （`crates/sla-checker/src/check_stmt.rs`），与 Zig 行为逐项一致；不替 Zig 改行为。

## 最小输入

```sla
fn f() -> int {
    let t = (2, 5);
    let (x, y) = t;
    let c = t;
    return x + y;
}
```

## 实证（原 Zig 插件，场地三 Kaggle 2202，插件树现头 6002b386，`sa sla check`）

探针 `p_destr_copy`：rc=1，`UseAfterMove`。`(2, 5)` 是全 Copy 元素元组，
普通 `let c = t;` 路径下 t 不会被消费（隐式移动站点有 Copy 豁免），
唯独解构路径消费了它。

## 机制（Zig 源码位置，本机参照树行号）

- `type_checker.zig:2913`（`let_destructure_stmt` 臂末尾）：对
  `rootIdentifier(value)` 无条件 `consumeBinding(root, "destructuring move")`，
  无 `typeIsCopy` / borrow 检查。
- 对照 `type_checker.zig:2844` 附近的普通 let 隐式移动站点：先判
  `!typeIsCopy(ty) and !isBorrowLikeType(ty)` 才消费——同一文件内两个
  相邻消费站点的豁免口径不一致。
- 元组类型本身按元素全 Copy 时是 Copy（`typeIsCopy` 的 tuple 分支），
  故该输入在 let 路径合法、解构路径被拒，属口径分叉而非语义必需。

## 建议修复形态（仅建议，不预判主线最终形态）

在 :2913 消费前补与 let 站点相同的豁免判定
（`typeIsCopy(root_ty)` 或 borrow-like 时跳过消费），或在缺陷登记处
明示解构恒移动的语义决定并同步 sala 文档——二者取一，不许保持
「相邻站点口径不一致且无文档」现状。

## Rust 侧单测锚点

`destructure_copy_tuple_still_consumes_root`（crates/sla-checker，
tests_phase2b）钉住当前 Zig 行为；Zig 侧若按建议修复，此测试的期望
应随差分复核翻转。

## 解决（2026-11-17，LINE-FB010）

Zig 现头（sa_plugin_sla 1f7c2f2c）的 `let_destructure_stmt` 臂已按本文件
建议形态补上同一豁免（臂内注 FB-010：`!typeIsCopy(sym.ty) and
!isBorrowLikeType(sym.ty)` 才消费）。Rust 侧随之镜像：checker
`check_let_destructure` 在消费根绑定前按根绑定自身类型判
`type_is_copy || is_borrow_like_type`，与 codegen 既有的 copy_root
跳释放口径对齐；单测锚点 `destructure_copy_tuple_still_consumes_root`
已翻转并更名为 `destructure_copy_tuple_keeps_root_usable`。语料
`test_unit_fb010_destructure_copy` 整名翻绿，负例
`test_error_fb010_destructure_noncopy` 仍按 UseAfterMove 拒绝。
