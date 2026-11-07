# 75 Weekend

对标 `tsgosa/demos/152_enum_calc`（`Sun=1`，`Mon=0`，`Sat=1`）。

- SLA 用 `enum + match` 表达（写法见 `15` 与 rosetta `60`，全臂列举避通配）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/75_weekend/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/75_weekend/main.sla
```
