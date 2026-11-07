# 67 Map Double

对标 `tsgosa/demos/193_map_dbl`（`[1,2,3]` 翻倍得 `[2,4,6]`）。

- 高阶 `map` 用显式循环 + `push` 表达（迭代器协议见路线图 Phase 6，写法见 `39/59`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/67_map_dbl/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/67_map_dbl/main.sla
```
