# 222 Btree Overwrite

对标 `tsgosa/demos/132_map_count` 的 BTree 版（102 为 HashMap 版）。

- 取-加-存覆写（`11`）+ 表长不变（`2`）；键须为字符串
  （int 键已转 loud 门，见 #30）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/222_btree_overwrite/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/222_btree_overwrite/main.sla
```
