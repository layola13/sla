# 55 Num Sep

对标 `tsgosa/demos/164_num_sep`（`1000000 / 1000 = 1000`）。

- 缺口 #11 已修复：`1_000_000` 可直写（词法保留数字间 `_`，转换点去分隔符）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/55_num_sep/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/55_num_sep/main.sla
```
