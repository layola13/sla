# 226 Idx Copy

对标 `tsgosa/demos/79_copy_loop`（和 `18`）。

- 与 84/89/98/203（`push` 式拷贝）互补，此处为定长目标下标写。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/226_idx_copy/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/226_idx_copy/main.sla
```
