# 117 Isqrt

对标 `tsgosa/demos/347_math_sqrt`（`4,3,1,10`）。

- SLA 尚无 `Math.sqrt`，用循环试乘表达整数开方下取整
  （与 49/52/113 的整数数学主题互补）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/117_isqrt/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/117_isqrt/main.sla
```
