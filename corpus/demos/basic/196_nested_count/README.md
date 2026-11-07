# 196 Nested Count

对标 `tsgosa/demos/1278_nested_loop`（`3x4=12`）。

- 与 146（嵌套 + `continue`）互补，此处为裸双层 `for in` 区间计数。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/196_nested_count/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/196_nested_count/main.sla
```
