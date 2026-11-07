# 359 sq sum

平方和循环版（`10 → 385`；27 的循环版，365 做三向）。

- 对标：27 sum_sq → 逐项累加（公式版留给 365 对峙）
- 绕行：无（纯 `int` while 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/359_sq_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/359_sq_sum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/359_sq_sum/main.sla -o /tmp/basic_359 && /tmp/basic_359
# 期望输出：385,30
```
