# 102 Map Count

对标 `tsgosa/demos/132_map_count`（`11, 2`，长 `2`）。

- `m.set("a", m.get("a") + 10)` 用取-加-存三步表达（覆盖写语义已验证）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/102_map_count/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/102_map_count/main.sla
```
