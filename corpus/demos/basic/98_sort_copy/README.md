# 98 Sort Copy

对标 `tsgosa/demos/26_sort`（原地 `6` + 拷贝 `11` + 原数组首元 `9`）。

- SLA 暂无 `toSorted`，先手工拷贝再排序表达同一语义（排序写法见 `16`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/98_sort_copy/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/98_sort_copy/main.sla
```
