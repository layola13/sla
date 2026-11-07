# 09 Vec Methods

对标 `tsgosa/demos/09_array_methods`（`push/map/filter/forEach`）。

SLA 用 `Vec::new/push` + `len(v)` + `v[i]` 表达；`map/filter` 的语义用显式
`while` 循环手写（`vec_sum_squares` ≈ map+reduce，`vec_count_even` ≈ filter+len），
避开 Phase 6 的 `for in` 迭代器协议缺口。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/09_vec_methods/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/09_vec_methods/main.sla
```
