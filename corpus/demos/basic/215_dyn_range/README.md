# 215 Dyn Range

动态上界区间求和（`10+20+30=60`）。

- 意对标 `tsgosa/demos/605_top_cond_forlen` 的“界取自集合长度”思想，
  原用例为字符串长（字符串区暂不碰），此处以 `len(v)` 作 `for in` 上界；
  此前区间上界皆为字面量（89/196/200/202/203），此处首覆动态界。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/215_dyn_range/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/215_dyn_range/main.sla
```
