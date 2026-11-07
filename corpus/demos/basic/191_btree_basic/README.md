# 191 Btree Basic

对标 `tsgosa/demos/132_map_count` 的 BTree 版（`1,2,0`）。

- basic 此前只有 HashMap 覆盖（101/102/105）与 fixture，
  此处为首个 BTreeMap 端到端 demo（`get().copied().unwrap_or_default()`
  口径见 #14）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/191_btree_basic/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/191_btree_basic/main.sla
```
