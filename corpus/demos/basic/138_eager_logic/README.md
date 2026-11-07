# 138 Eager Logic

对标 `tsgosa/demos/643_logic_short`（结果 `0,3`，但两侧各执行一次）。

- SLA `&&`/`||` 为急求值（eager）：`false && eff()` 与 `true || eff()`
  仍执行右侧（exe 打印两次 EFF，与 TS 短路语义不同，见缺口 #21）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/138_eager_logic/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/138_eager_logic/main.sla
```
