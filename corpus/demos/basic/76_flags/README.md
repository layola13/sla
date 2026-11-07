# 76 Flags

对标 `tsgosa/demos/106_flags`（`perm=7`；`2, 0, 5`）。

- 与 `44`（拆位计数）互补，此处为 `|` 组合 + `&` 测试 + `^` 翻转。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/76_flags/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/76_flags/main.sla
```
