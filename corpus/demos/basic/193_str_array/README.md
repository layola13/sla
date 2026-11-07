# 193 Str Array

对标 `tsgosa/demos/286_str_elem` 的读值部（`1,2`）。

- 定长字符串数组 + `str_eq` 断言（与 95/122 的字符串口径一致；
  `split` 等方法暂无，此处只做存取）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/193_str_array/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/193_str_array/main.sla
```
