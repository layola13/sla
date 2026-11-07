# 131 Arr Alloc

对标 `tsgosa/demos/446_new_array_n`（`60,3`）。

- 定长数组长度即类型一部分（`[i32; 3]`），无 `new Array(n)` 构造，
  用字面量占位 + 下标写表达（与 08/28 的数组口径一致）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/131_arr_alloc/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/131_arr_alloc/main.sla
```
