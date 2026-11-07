# 156 Copywithin

对标 `tsgosa/demos/981_copywithin`（`3,4`）。

- SLA 无 `copyWithin` 方法，用显式循环前向覆盖表达（与 89/103 的
  手工切片口径一致；源区间在目标之后，前向即安全）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/156_copywithin/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/156_copywithin/main.sla
```
