# 473 duel pent

五边形数双实现对峙（闭式 vs 差分累加；460 的多边形版）。

- 对标：460 duel_lazy → `Σ(3k-2)` 锁定（`5 → 35=35`）
- 绕行：无（首项 1；纯 `int` 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/473_duel_pent/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/473_duel_pent/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/473_duel_pent/main.sla -o /tmp/basic_473 && /tmp/basic_473
# 期望输出：35,35
```
