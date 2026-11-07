# 64 Index Select

对标 `tsgosa/demos/188_select`（`a[1]=20`，`a[2]=30`）。

- 定长 `[int; 3]` 字面量（类型口径见 `POTENTIAL_ISSUES.md` #1：统一用 `int`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/64_select/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/64_select/main.sla
```
