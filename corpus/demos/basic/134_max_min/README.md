# 134 Max Min

对标 `tsgosa/demos/511_math_int` 的 max/min 部（`7,3,5,2`）。

- SLA 尚无 `Math.max/min`，用分支表达（与 34 的 pick-max 同构，
  此处覆盖双函数 + 极值相等分支）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/134_max_min/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/134_max_min/main.sla
```
