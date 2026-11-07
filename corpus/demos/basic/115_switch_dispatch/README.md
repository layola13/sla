# 115 Switch Dispatch

对标 `tsgosa/demos/271_switch_arms`（`12,100`）。

- switch 写法与 33/107 同口径；条件链用显式 `else if`
  （与 04/34 的分支口径一致）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/115_switch_dispatch/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/115_switch_dispatch/main.sla
```
