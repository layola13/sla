# 144 Struct Bool

对标 `tsgosa/demos/790_top_obj_bool`（`3,3`）。

- 原用例为顶层常量（SLA 顶层只容 `const` 且结构体常量不可用，见 #19），
  此处用局部绑定表达（与 114 的整数字段互补，124 覆盖布尔数组）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/144_struct_bool/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/144_struct_bool/main.sla
```
