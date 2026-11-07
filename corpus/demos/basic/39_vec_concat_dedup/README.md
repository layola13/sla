# 39 Vec Concat Dedup

对标 `tsgosa/demos/136_concat_all`（`concat`）+ `117_dedup`（`indexOf+push` 去重）。

SLA 的 `concat/indexOf` 用显式循环表达：拼接即逐个 `push`，
去重用手写 `vec_contains` 线性查找（`[1,2,2,3,1] -> [1,2,3]`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/39_vec_concat_dedup/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/39_vec_concat_dedup/main.sla
```
