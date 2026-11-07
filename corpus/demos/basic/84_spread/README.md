# 84 Spread

对标 `tsgosa/demos/29_spread_elem`（展开长 `4` + 尾元 `4` = `8`）。

- SLA 暂无展开语法，用拷贝循环 + `push` 表达（写法见 `39/53/83`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/84_spread/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/84_spread/main.sla
```
