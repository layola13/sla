# 35 Min Range

对标 `tsgosa/demos/116_minloop` + `129_range`。

- `min_of`：`for` 扫描最小值（`22_second_max` 的简化版）。
- `range_sum`：半开区间 `[1, 4)` 求和（`20+30+40=90`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/35_min_range/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/35_min_range/main.sla
```
