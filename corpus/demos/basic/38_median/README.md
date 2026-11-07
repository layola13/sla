# 38 Median

对标 `tsgosa/demos/147_median`（`toSorted()[2]` + 首尾和）。

SLA 暂无 `toSorted`，用 `16_insert_sort` 的插入排序实现同一语义
（中位数 `5`，`b[0]+b[4] = 1+9 = 10`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/38_median/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/38_median/main.sla
```
