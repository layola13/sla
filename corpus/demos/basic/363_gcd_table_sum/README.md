# 363 gcd table sum

gcd 表和（`m=12,n=6 → 17`；125 表版的 gcd 版）。

- 对标：125 表 → `gcd(i,m)` 逐项累加（`1+2+3+4+1+6`）
- 绕行：无（纯 `int` 循环复用 `gcd`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/363_gcd_table_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/363_gcd_table_sum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/363_gcd_table_sum/main.sla -o /tmp/basic_363 && /tmp/basic_363
# 期望输出：17,8
```
