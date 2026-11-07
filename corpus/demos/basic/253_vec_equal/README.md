# 253 Vec Equal

Vec 逐元相等（167 为引用恒等版：等 `1`、不等 `0`、长异 `0`）。

- 注意值语义：Vec 值参跨语句复用即搬移 trap（见 #32），比较以内联写。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/253_vec_equal/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/253_vec_equal/main.sla
```
