# 16 Insert Sort

对标 `tsgosa/demos/102_insert_sort`。

`for i in 1..5` + `while j >= 0 && a[j] > k` 内移，形参 `^a: [int; 5]` 原地写。
注意元素类型统一用 `int`（见 POTENTIAL_ISSUES #1）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/16_insert_sort/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/16_insert_sort/main.sla
```
