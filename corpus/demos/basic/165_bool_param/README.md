# 165 Bool Param

对标 `tsgosa/demos/1044_bool_param`（`0,1`）。

- 条件须严格布尔（129），三元 `b ? 0 : 1` 写法见 06。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/165_bool_param/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/165_bool_param/main.sla
```
