# 109 F64 Neg

对标 `tsgosa/demos/313_fneg`（`11`）。

- f64 取负 + 比较，与 90/108 同口径：字面量比较 + 变量比较双覆盖。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/109_f64_neg/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/109_f64_neg/main.sla
```
