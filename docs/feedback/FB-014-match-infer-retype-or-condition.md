# FB-014 — match 推断路径以「或」条件改写 scrutinee 绑定类型

- **反馈环批次**：第二批（checker 二期扩围线，2026-11-07）
- **源登记**：`docs/divergences/checker.md` D-C16（checker 线）
- **分类**：Zig 侧缺陷候选 —— 源码直读确认（启发式过宽 + 检查器改写已定义绑定类型）
- **Rust 侧处置**：只镜像 + 登记。Rust `check_match` 的推断路径
  （`crates/sla-checker/src/check_match.rs`）按 Zig 原样实现，包括
  对 scrutinee 绑定类型的改写。

## 机制（Zig 源码位置，本机参照树行号）

- `type_checker.zig:5494` 起 `match_expr` 臂的推断路径：scrutinee
  类型为 `infer` 且 cases 非空时，由**第一个** case 模式决定目标类型，
  判定条件是析取（OR）：
  - `enum_name == "Option"` **或** 变体名是 `Some`/`None` → 按 `Option<Infer>` 处理；
  - `enum_name == "Result"` **或** 变体名是 `Ok`/`Err` → 按 `Result<Infer, Infer>` 处理；
  - 否则按模式的 `enum_name` 查枚举声明（查不到才 TypeMismatch）。
- 即：任一用户自定义枚举只要有一个变体名为 `Some`（枚举名并非
  Option），其 infer 型 scrutinee 就会被当作 Option 处理。
- 路径末尾还会**改写** scrutinee 根绑定的 `sym.ty` 为推断类型并
  重记 expr 类型——检查器在此处修改已定义绑定的类型，后续对该
  绑定的使用按新类型解释，与声明/初值来源脱钩。

## 建议修复形态（仅建议，不预判主线最终形态）

判定收紧为合取或精确匹配：只有 `enum_name` 精确等于 `Option` /
`Result` 时才走内建路径，变体名单独出现时落回枚举声明查找；
scrutinee 类型的改写若保留，应在 sala/设计层明示为「match 推断
回填」语义并给出作用域说明。本单只登记机制，不预判收紧后的
兼容面影响。

## Rust 侧单测锚点

`match_option_some_binds_payload` 等 Option/Result 匹配测试
（tests_phase2）钉住内建路径行为；OR 条件误判形态目前无独立
探针，Zig 侧修复时应补最小复现并与 Rust 侧对拍。
