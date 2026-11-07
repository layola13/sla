# 224 Nest4

对标 `tsgosa/demos/916_nested_struct` 的四层延伸（`7`）。

- 195 覆盖三层时注明“四层以上未探”，此处补齐穿透读。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/224_nest4/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/224_nest4/main.sla
```
