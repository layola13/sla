# 218 Nested Calls

对标 `tsgosa/demos/1062_nested_calls`（`add(mul(2,3),mul(4,5))=26`）。

- 与 189（调用结果组数组）互补，此处为实参位嵌套。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/218_nested_calls/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/218_nested_calls/main.sla
```
