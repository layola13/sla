# 78 Map Filter

对标 `tsgosa/demos/114_compose`（`map(+1)` 后 `filter(>2)`，长 `2` 首元 `3`）。

- 高阶链用两轮显式循环 + `push` 表达（迭代器协议见路线图 Phase 6，写法见 `39/67`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/78_map_filter/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/78_map_filter/main.sla
```
