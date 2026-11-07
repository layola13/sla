# 65 Sort Slice

对标 `tsgosa/demos/178_obj_sum`（排序 `[3,1,2]→[1,2,3]` + 切片 `[2,3]`）。

- 排序用插入排序（写法见 `16`）；SLA 暂无 `slice` 方法，切片用手工拷贝表达。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/65_sort_slice/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/65_sort_slice/main.sla
```
