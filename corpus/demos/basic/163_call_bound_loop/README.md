# 163 Call Bound Loop

对标 `tsgosa/demos/1057_for_call_bound`（`6`）。

- SLA `for` 无步长且界须为区间，此处用 `while i < n()` 表达
  （每次迭代重求上界，与 TS 语义一致）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/163_call_bound_loop/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/163_call_bound_loop/main.sla
```
