# 187 Two Structs

对标 `tsgosa/demos/791_top_obj_multi`（`5,5`）。

- 原用例为顶层常量（顶层结构体常量不可用，见 #19），此处用局部绑定
  表达（与 144 的单结构体互补）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/187_two_structs/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/187_two_structs/main.sla
```
