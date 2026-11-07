# 371 lcm table sum

lcm 表和（`m=4,n=4 → 24`；348 的表版）。

- 对标：348 lcm_chain → 逐项 `lcm(i,m)` 累加（`4+4+12+4`）
- 绕行：无（先除后乘防溢出；纯 `int` 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/371_lcm_table_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/371_lcm_table_sum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/371_lcm_table_sum/main.sla -o /tmp/basic_371 && /tmp/basic_371
# 期望输出：24,12
```
