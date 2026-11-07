# 06 Ternary

对标 `tsgosa/demos/06_ternary` + `153_ternary_chain`。

SLA 没有 `?:` 三元运算符，用 `if` 表达式表达（`test_unit_if_else_expr` 已锁定该语义）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/06_ternary/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/06_ternary/main.sla
```
