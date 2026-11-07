# 201 Pop Drain

对标 `tsgosa/demos/1416_pop_cond`（排空求和 `5`）。

- `len(v)` 驱动循环 + `pop().unwrap()`（与 141 的 `unwrap` 口径一致）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/201_pop_drain/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/201_pop_drain/main.sla
```
