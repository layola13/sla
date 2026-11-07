# 151 Switch Call

对标 `tsgosa/demos/902_switch_call`（`2,0`）。

- switch 直接以调用结果作 scrutinee，写法与 33/107 同口径。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/151_switch_call/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/151_switch_call/main.sla
```
