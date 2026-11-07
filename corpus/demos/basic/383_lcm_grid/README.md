# 383 lcm grid

lcm 网格和（`3×3 → 28`；370 的 lcm 版）。

- 对标：370 gcd_grid_sum → 双层 while 网格（`1+2+3+2+2+6+3+6+3`）
- 绕行：无（先除后乘防溢出；嵌套循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/383_lcm_grid/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/383_lcm_grid/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/383_lcm_grid/main.sla -o /tmp/basic_383 && /tmp/basic_383
# 期望输出：28,7
```
