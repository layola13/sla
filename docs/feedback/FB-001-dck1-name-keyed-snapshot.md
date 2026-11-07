# FB-001 — type_checker 分支快照按名字键控：遮蔽绑定下 use-after-move 漏检

- **反馈环批次**：第一批（用户 2026-11-07 常设政策：Rust 侧发现 → Zig 侧同步修复）
- **主线候选名**：D-CK1（主会话已登记）
- **源登记**：`docs/divergences/checker.md` D-C1（checker 线）；合编 `docs/DIVERGENCES.md`
- **分类**：Zig 侧缺陷候选 —— **已实证确认**（见下）
- **Rust 侧处置**：只镜像 + 登记。Rust 检查器以 `BindingId` 身份键控快照
  （`crates/sla-checker/src/scope.rs`），对最小输入报 UseAfterMove；不替 Zig 改行为。

## 最小输入

`docs/probes/dc1_shadow.sla`（按 Zig 文法在 if-else 语句尾补了 `;`，
无分号形态在 Zig parser 处先报 SyntaxError、到不了检查器；语义场景不变）：

```sla
fn f(s: String, t: String, flag: bool) {
    {
        let s = t;                 // 内层 s 遮蔽参数 s
        if flag { let a = s; }     // then 分支消费内层 s
        else { };
        let b = s;                 // then 路径上内层 s 已被移动
    }
}
```

## 实证（原 Zig 插件，场地 Kaggle 2201，插件树 699f13d4，`sa sla check`）

证据原文：`docs/probes/dc1_repro.out.txt`。三路对照：

| 输入 | 场景 | Zig 结果 |
|---|---|---|
| dc1_floor | 无条件双重移动（无 if） | `UseAfterMove` rc=1 |
| dc1_control | 条件消费、**无遮蔽** | `UseAfterMove` rc=1 |
| dc1_shadow | 条件消费、**有遮蔽**（本单输入） | **rc=0 放行（漏检）** |

control 与 shadow 的唯一差别就是遮蔽：无遮蔽抓得住、有遮蔽漏检，
与下述源码机制预测逐项吻合。

## 机制（Zig 源码位置，本机参照树行号）

- `type_checker.zig:2712` `saveScopeStates`：由内向外逐层把
  `states.put(name, state)` 写进**一张扁平** `StringHashMap(ValueState)`，
  同名键被**外层**绑定状态覆盖——快照里一个名字只剩最外层绑定的状态。
- `type_checker.zig:2725` `restoreScopeStates`：把这一个状态写回**每一层**
  的同名绑定。
- `type_checker.zig:5366` 起：if 臂的 then 快照是同一循环的手工内联复制
  （未走 saveScopeStates），覆盖顺序相同。
- `type_checker.zig:2738` `mergeMultiBranchScopeStates`：按名读
  `states.get(name) orelse .active` 做分支合并，同样合并同名绑定。
- 连带：cleanups / phi_cleanups 侧表的值是**名字列表**，块退出释放发射
  同样按名寻址——遮蔽场景下不止状态记账，释放对象也可能按名错位。

本输入中：then 分支消费的是内层 `s`，但 then 快照里 `"s"` 被参数 `s`
的 Active 覆盖；合并时两侧都是 Active → 消费事实丢失 → 事后读放行。

## 建议修复形态（仅建议，不预判主线最终形态）

快照 / 合并 / 清理三处一并由「名字键控」改为「绑定身份键控」
（作用域帧 + 帧内序号之类的稳定身份）。只改其中一处，仍会留下
按名释放错对象或状态写回错层的问题。三处的完整调用点清单见源登记。

## 复证（现头口径，2026-11-07 场地三）

同一组三件在场地三以**现头 sla 6002b386** 复跑：floor rc=1、
control rc=1、shadow **rc=0**，与 699f13d4 结果逐项一致——
漏检在现头仍成立，本单证据自此按现头口径计。

## Rust 侧单测锚点

`shadowing_snapshot_is_per_binding`（crates/sla-checker）钉住 Rust 侧判定，
供修复后差分复核：同一输入在修复后的 Zig 侧应报 UseAfterMove。
