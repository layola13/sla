# FB-009 — shouldAutoBorrowReceiverArg 冗余死分支（严重度：琐细）

- **反馈环批次**：第二批（lowering/text 线 (c) 类）
- **源登记**：`docs/divergences/lowering-text.md` D-T8；合编 `docs/DIVERGENCES.md`
- **分类**：Zig 侧缺陷候选 —— 死代码/重构残留，**行为等价、无功能影响**
- **Rust 侧处置**：原样镜像（`crates/sla-codegen-text` call_args.rs），行为一致。

## 机制（Zig 源码位置）

- `src/lowering_rules.zig:6221-6226`：`shouldAutoBorrowReceiverArg`
  内层有 `if (arg_ty is borrow) return true;`，其后无条件
  `return true` —— 内层判定对结果无任何影响。

## 影响

- 无行为影响。风险仅在可读性/审计面：读者会以为 borrow 形态
  有一条独立判定路径，后续修改者可能在死分支上做无效修改，
  或误判该函数的真实判定结构。疑似重构残留。

## 建议修复形态（仅建议，不预判主线最终形态）

删除死分支、保留单一 return true（并视需要以注释说明该判定
当前恒真的理由）；或若原意是 borrow 与非 borrow 分流，则
补回缺失的 else 语义并以用例钉住。属顺手清理级，不必单开线，
可并入 lowering 相关修复批。

## Rust 侧锚点

`crates/sla-codegen-text` call_args.rs 原样镜像并注源行号；
主线清理后 Rust 侧同步简化，行为不变、无需差分复验。
