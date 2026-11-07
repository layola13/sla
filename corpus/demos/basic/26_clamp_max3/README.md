# 26 Clamp Max3

对标 `tsgosa/demos/126_clamp`（`Math.min/max` 版改写为 `if` 表达式链）。

SLA 无 `Math` 全局对象，`clamp/max3` 用嵌套 `if/else` 表达式表达，
与 `06_ternary` 的 `pick_chain` 同构。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/26_clamp_max3/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/26_clamp_max3/main.sla
```
