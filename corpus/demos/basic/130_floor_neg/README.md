# 130 Floor Neg

对标 `tsgosa/demos/497_math_floor_neg` + `545_f64round`（`7,-8,3,-7,2`）。

- SLA 尚无 `Math.floor/ceil/trunc/round`，用分支 + `as i32` 截断表达；
  100 覆盖正数 abs + 截断，此处主覆盖负数分支。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/130_floor_neg/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/130_floor_neg/main.sla
```
