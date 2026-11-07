# 113 Pow Loop

对标 `tsgosa/demos/273_pow_assign`（`8,1`）。

- SLA 尚无 `**` / `**=`（`**` 解析为非法解引用），此处用循环连乘
  表达（与 49_pow2/52_pow2_series 的幂主题互补）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/113_pow_loop/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/113_pow_loop/main.sla
```
