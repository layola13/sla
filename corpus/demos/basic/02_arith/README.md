# 02 Arith

对标 `tsgosa/demos/02_arith` + `21_compound`。

覆盖 `+ - * / %`、括号优先级、一元负号。SLA 整除语义与 TS 一致（i32 整除）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/02_arith/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/02_arith/main.sla
```
