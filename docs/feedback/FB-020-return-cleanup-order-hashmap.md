# FB-020 — return 出口清理名单顺序取自 StringHashMap 迭代序（清理顺序无规约序）

- **反馈环批次**：二期 oracle 线（用户 2026-11-07 常设政策：Rust 侧发现 → Zig 侧同步修复）
- **源登记**：`docs/divergences/oracle-p2.md` D-O1；与已在册 **checker D-C6**
  （块退出清理名单顺序 = StringHashMap 哈希迭代序，顺序敏感性未证实）同源，
  本单是其首个字节级实证，并把「顺序敏感性」从未证实推进到「产物字节可观察」。
- **分类**：Zig 侧缺陷候选 —— **已实证**（见下）。定性候主会话确认：
  若 Zig 侧认定清理顺序即应为规约序（如声明序），则 Zig 侧为缺陷；
  Rust 侧按全线纪律「发射路径禁消费 HashMap 迭代顺序」不迁就哈希序。
- **Rust 侧处置**：只镜像语义 + 登记。Rust checker 清理名单按声明序
  （a、b、c、d）生成，发射确定性；不替 Zig 改行为。

## 最小输入

`docs/probes/e2e_a.sla`（无导入、无 std 表面，端到端可跑）：

```sla
fn arith_values() -> i32 {
    let a = 2 + 3 * 4;
    let b = (20 - 8) / 3;
    let c = 17 % 5;
    let d = -(2 + 3);
    return a + b * 10 + c * 100 + d;
}
```

## 实证（原 Zig 插件，场地 Kaggle 2202，sa 0.1.8，`sa sla build`）

证据原文：`results/p2/e2e_a/zigwork/e2e_a.sa`（Zig 产物）与
`results/p2/e2e_a/run1/out.sa`（Rust 产物）。`arith_values` 的
return 前清理段：

| 实现 | 清理序 |
|---|---|
| Zig | `!tmp_23`、`!b`、`!a`、`!c`、`!d`、return |
| Rust | `!tmp_23`、`!a`、`!b`、`!c`、`!d`、return |

两产物 diff 全文仅此一处顺序差 + @test 体裁剪口径差（D-O2，另行
登记）。对照组 e2e_b `while_sum`（绑定 t、i）：两边顺序恰同、
函数体逐字节无差——分歧只在哈希序 ≠ 声明序时显形，与机制吻合。

## 机制（候主线核对）

checker D-C6 登记：Zig 块退出清理名单由 StringHashMap 收集，
迭代序即发射序。本例四绑定哈希序把 b 排到 a 前。清理顺序在
非 Copy 类型的 drop 场景可观察（析构副作用次序），且使产物字节
依赖哈希实现细节，差分与复现构建均受其扰。

## 建议修复形态（仅建议，不预判主线最终形态）

清理名单收集改按绑定身份/声明序的确定性容器（如按作用域帧内
序号排序），使 Zig 产物清理序 = 声明序，与 Rust 侧现行口径合流；
修复后以 e2e_a 双边对拍复证（预期 diff 仅剩 D-O2 的 @test 段）。
