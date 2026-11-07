# 94 Nested Template

对标 `tsgosa/demos/105_nested_tpl`（`sum=7 prod=12`）。

- 模板断言用 `str_eq`；缺口 #4b 已修复，`println(format(...))` 直打与赋值后
  打印均正常（`check` 侧多插值已修复，见 #4）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/94_nested_tpl/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/94_nested_tpl/main.sla
```
