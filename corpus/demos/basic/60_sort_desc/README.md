# 60 Sort Desc

对标 `tsgosa/demos/190_sort_desc`（`[1,2,3,4,5]` 降序，首 `5` 尾 `1`）。

- `16/38` 已有升序插入排序，此处取反比较子（`a[j] < k`）做降序版，
  定长 `[int; 5]` 原地写法与 `16` 一致。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/60_sort_desc/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/60_sort_desc/main.sla
```
