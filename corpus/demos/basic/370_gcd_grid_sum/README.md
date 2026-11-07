# 370 gcd grid sum

gcd 网格和（`3×3 → 12`；363 的网格版）。

- 对标：363 gcd_table_sum → 双层 while 网格（`1+1+1+1+2+1+1+1+3`）
- 绕行：无（嵌套循环复用 `gcd`；48_nested 绕行口径一致）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/370_gcd_grid_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/370_gcd_grid_sum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/370_gcd_grid_sum/main.sla -o /tmp/basic_370 && /tmp/basic_370
# 期望输出：12,5
```
