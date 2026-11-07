# 146 Nested Continue

对标 `tsgosa/demos/893_nested_continue`（`8`）。

- SLA 有 `continue`（探针与 TS 语义一致）；label 跳出不支持（#9），
  此处只用普通 continue（32 覆盖 break）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/146_nested_continue/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/146_nested_continue/main.sla
```
