# 507 fibcode

斐波那契编码长（`100 → 11`；506 的编码版）。

- 对标：506 zeckendorf → 最大项序号即码长（`10 → 6`，`1 → 2`）
- 绕行：`n==1` 特判返 2（506 同例；序号从 F₂ 起）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/507_fibcode/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/507_fibcode/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/507_fibcode/main.sla -o /tmp/basic_507 && /tmp/basic_507
# 期望输出：11,6,2
```
