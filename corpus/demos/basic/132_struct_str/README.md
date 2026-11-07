# 132 Struct Str

对标 `tsgosa/demos/552_strobj`（`2,3`）。

- 字符串字段须注解 `: ptr`（见 #3/#5），`len()` 读 Slice 长度（见 #10）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/132_struct_str/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/132_struct_str/main.sla
```
