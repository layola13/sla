# 506 zeckendorf

Zeckendorf 项数（`100 → 3`；237 的贪心版）。

- 对标：237 fib_table → 最大斐波那契贪心剥离（`10 → 2`）
- 绕行：`m==1` 特判（`1,2` 双初值循环 `1` 会越过；407 类守卫）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/506_zeckendorf/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/506_zeckendorf/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/506_zeckendorf/main.sla -o /tmp/basic_506 && /tmp/basic_506
# 期望输出：3,2,1
```
