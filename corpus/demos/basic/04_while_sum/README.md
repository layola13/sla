# 04 While Sum

对标 `tsgosa/demos/04_while_sum` + `110_while_break`（前者）。

`while i < limit` 累加。注意 SLA 中 `let` 绑定默认不可变再赋值，
需要可变累加时直接用 `let t = ...; t = t + i;`（SLA 允许同作用域再赋值，
与 `var` 的区别见 `test_error_var_*`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/04_while_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/04_while_sum/main.sla
```
