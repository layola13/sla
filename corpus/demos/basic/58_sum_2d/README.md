# 58 Sum 2d

对标 `tsgosa/demos/172_sum_2d`（`[[1,2,3],[4,5,6]]` 求和得 `21`）。

- SLA 无嵌套数组字面量，按 `19/28` 的行主序扁平布局用 `[int; 6]` + 双层循环。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/58_sum_2d/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/58_sum_2d/main.sla
```
