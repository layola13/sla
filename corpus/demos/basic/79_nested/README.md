# 79 Nested Struct

对标 `tsgosa/demos/33_nested_struct`（`40+2=42`）。

- 原用例另含 `name` 字符串字段，SLA 结构体字符串字段未经验证，
  此处只取整数嵌套核，字符串语义见 `10/68`。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/79_nested/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/79_nested/main.sla
```
