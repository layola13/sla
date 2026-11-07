# 139 Discard Loop

对标 `tsgosa/demos/672_void_i32` + `676_void_loop`（`7,7,1`）。

- 返回值可直接弃置（`seven();` 独立成句，check 放行，无 `void` 运算符）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/139_discard_loop/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/139_discard_loop/main.sla
```
