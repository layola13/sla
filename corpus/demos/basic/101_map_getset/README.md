# 101 Map Getset

对标 `tsgosa/demos/35_map_getset`（存取求和 `3` + 缺键 `0` + 表长 `2`）。

- `has(c)` 用缺键取零表达（`get().copied().unwrap_or_default()`，见缺口 #14）；
  `getSize()` 用 `len(m)` 表达（SAB 经 `MAP_LEN` 直降）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/101_map_getset/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/101_map_getset/main.sla
```
