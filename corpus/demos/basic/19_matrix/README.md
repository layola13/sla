# 19 Matrix

对标 `tsgosa/demos/143_mat_add` + `144_transpose`。

2x2 矩阵按行主序扁平为 `[int; 4]`（`[a00, a01, a10, a11]`），
加法逐元相加，转置求和显式按 `[a0, a2, a1, a3]` 索引以锁定布局语义。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/19_matrix/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/19_matrix/main.sla
```
