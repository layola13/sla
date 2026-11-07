# 88 Grade

对标 `tsgosa/demos/23_switch_plain`（`1, 2, 3`）。

- 与 `13`（表达式式分类）/`33`（混合 return 臂）互补，此处为纯全 `return` 臂
  写法（#7 已修复，探针 p7a 同款）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/88_grade/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/88_grade/main.sla
```
