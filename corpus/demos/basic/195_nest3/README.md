# 195 Nest3

对标 `tsgosa/demos/916_nested_struct` 的三层版（`9`）。

- 79 覆盖两层嵌套读，此处为三层内联字面量 + 穿透读
  （探针已验证三线；四层以上未探，按需再探）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/195_nest3/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/195_nest3/main.sla
```
