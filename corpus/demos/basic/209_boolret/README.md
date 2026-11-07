# 209 Boolret

对标 `tsgosa/demos/567_boolret`（`eq(1,1)` 真）。

- TS 用 `===`（SLA 无该 token），此处用 `==`；与 165（布尔形参）互补。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/209_boolret/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/209_boolret/main.sla
```
