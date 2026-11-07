# 105 Map Clear

对标 `tsgosa/demos/167_map_clear`（清空后表长 `0`）。

- `clear()` 经 `MAP_CLEAR` 直降（checker 定 `void` 类型，双后端一致）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/105_map_clear/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/105_map_clear/main.sla
```
