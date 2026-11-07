# 116 Linear Search

对标 `tsgosa/demos/323_unannot_scan`（`2,1,3`，和 312）。

- SLA 无 `indexOf/includes/lastIndexOf` 方法，用显式循环 + 索引表达
  （与 17/21 的查找口径一致）。
- 注意 #1：形参为 `[i32; 4]` 时，实参须显式注解（字面量推断 `int`
  与 `i32` 宽度不一致，check 直接拦截）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/116_linear_search/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/116_linear_search/main.sla
```
