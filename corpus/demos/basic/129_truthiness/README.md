# 129 Truthiness

对标 `tsgosa/demos/442_boolean_ctor`（真值 `1,0,5`）。

- SLA 条件须为严格布尔（`if (x)` 直接 TypeMismatch），真值语义用
  显式 `!= 0` 表达（与 14 的布尔口径一致）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/129_truthiness/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/129_truthiness/main.sla
```
