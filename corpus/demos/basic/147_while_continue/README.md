# 147 While Continue

对标 `tsgosa/demos/895_while_continue`（`37`）。

- 注意：`continue` 前须先步进循环变量，否则与 TS 同样死循环
  （语义一致，非缺口）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/147_while_continue/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/147_while_continue/main.sla
```
