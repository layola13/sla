# 160 Rev Index Sum

对标 `tsgosa/demos/1052_rev_index`（`6`）。

- 倒序遍历（`len-1` 起向下），与正序求和（05/58）方向互补。
- 注意：`len()` 为无符号语义，直接 `let i = len(a) - 1` 会使
  `i >= 0` 恒真、递减回绕崩溃（signal 11）；须先 `as i32`
  转有符号再倒数（103 同款写法）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/160_rev_index_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/160_rev_index_sum/main.sla
```
