# 229 Nest Vec

嵌套 Vec 面（58 扁平行主序的嵌套版；意对标 `tsgosa/demos/172_sum_2d` 的布局思想）。

- 存（`1`）+ 子表长（`2`）+ 穿透读求和（`3`）；定长嵌套读见缺口 #20，
  Vec 嵌套三线全绿。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/229_nest_vec/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/229_nest_vec/main.sla
```
