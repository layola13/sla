# 180 Swbind Str

对标 `tsgosa/demos/435_str_switch`（`2,9`）。

- 126 只用字面量 scrutinee；此处探针证实 `: ptr` 绑定 scrutinee
  分发正常（相等路径），失真仅限关系比较（见 #18）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/180_swbind_str/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/180_swbind_str/main.sla
```
