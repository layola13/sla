# 217 Intmap

对标 `tsgosa/demos/35_map_getset` 的 int 键版（101 为 string 键版）。

- 存取 + 缺键取零 + 表长 + 覆写（102 的取-加-存口径）；
  BTree 的 int 键见缺口 #30（已转 loud 门，此处只用 HashMap）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/217_intmap/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/217_intmap/main.sla
```
