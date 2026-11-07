# 20 Closures

对标 `tsgosa/demos/109_closure` + `24_find_some_every`。

- 闭包捕获外层 `offset`（见 `tests/test_unit_closures.sla`）。
- `find/some/every` 用显式 `while` 循环手写（`vec_find_gt/vec_some_gt/vec_every_gt`），
  语义对齐 tsgosa 的 `[3,1,4,1,5]` 用例（`find >3 = 4`，`some >4 = true`，`every >0 = true`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/20_closures/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/20_closures/main.sla
```
