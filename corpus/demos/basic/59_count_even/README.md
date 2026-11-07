# 59 Count Even

对标 `tsgosa/demos/174_count_even`（`[1..8]` 偶数计数得 `4`）。

- 与 `29/43`（定长数组版）互补，此处换 `Vec` 版（`len` + 索引循环）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/59_count_even/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/59_count_even/main.sla
```
