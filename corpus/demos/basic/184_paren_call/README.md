# 184 Paren Call

对标 `tsgosa/demos/481_paren_call`（`42,3`）。

- `(add)(...)` 括号包裹被调函数双后端正常（探针 + exe 核对）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/184_paren_call/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/184_paren_call/main.sla
```
