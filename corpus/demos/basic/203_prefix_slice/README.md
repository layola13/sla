# 203 Prefix Slice

对标 `tsgosa/demos/1423_slice_0_2`（前缀长 `2`，次元 `2`，原表不动）。

- SLA 无 `slice` 方法（见 83/89），用 `for in` 区间逐元 `push`；
  与 89 的尾段版互补，此处为前缀版。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/203_prefix_slice/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/203_prefix_slice/main.sla
```
