# 23 Rotate

对标 `tsgosa/demos/122_rotate`（`slice+concat` 版）。

SLA 暂无 `slice/concat` 快捷语义，用取模索引构造旋转后的定长数组。
`Vec` 版的动态旋转可参考 `09_vec_methods` 自行扩展。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/23_rotate/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/23_rotate/main.sla
```
