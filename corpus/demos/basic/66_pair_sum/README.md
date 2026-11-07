# 66 Pair Sum

对标 `tsgosa/demos/194_class_pair`（`Pair(11,22)`，和方法 `33`）。

- SLA 用 `struct + impl` 表达类（与 `11/12` 一致），此处为双字段 + 读方法。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/66_pair_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/66_pair_sum/main.sla
```
