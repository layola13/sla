# 199 Dowhile Continue

对标 `tsgosa/demos/1395_dowhile_continue` 语义（`1+3+4=8`）。

- SLA 无 `do-while` 语法，用 `while true` + 尾 `break` 表达同一语义
  （与 40 同口径）；`continue` 前已步进，与 147 一致。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/199_dowhile_continue/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/199_dowhile_continue/main.sla
```
