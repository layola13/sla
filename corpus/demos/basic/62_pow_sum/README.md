# 62 Pow Sum

对标 `tsgosa/demos/176_pow_sum`（`1+2+4+8=15`）。

- SLA 无 `**` 运算符（与 `52_pow2_series` 一致），用翻倍累乘表达。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/62_pow_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/62_pow_sum/main.sla
```
