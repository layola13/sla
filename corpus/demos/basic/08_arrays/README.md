# 08 Arrays

对标 `tsgosa/demos/08_arrays`。

SLA 定长数组 `[3, 1, 4, 1, 5]` 推导为 `[i32; 5]`，`len(a)` 取长度，
`a[i] = v` 原地写（见 `tests/test_unit_arrays.sla`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/08_arrays/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/08_arrays/main.sla
```
