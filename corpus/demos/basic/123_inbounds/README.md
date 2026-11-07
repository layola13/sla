# 123 Inbounds

对标 `tsgosa/demos/387_inarr`（`1,0,1,1,0`）。

- SLA 无 `in` 运算符，用显式区间判断 `0 <= i && i < len` 表达。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/123_inbounds/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/123_inbounds/main.sla
```
