# 108 F64 Loop

对标 `tsgosa/demos/279_f64_loop`（`n=3, m=7`）。

- SLA `for i in 0..N` 为整数区间，浮点步进用 `while` 显式累加
  （与 90_f64_cmp 的 f64 比较口径一致）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/108_f64_loop/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/108_f64_loop/main.sla
```
