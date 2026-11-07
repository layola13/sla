# 148 Call Len

对标 `tsgosa/demos/898_chain_call_len`（`2`）。

- 返回 `: ptr` 字符串再 `len()` 双后端正常；返回未注解 `String`
  则落入缺口 #15（TypeMismatch），此处只用 `: ptr` 标注。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/148_call_len/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/148_call_len/main.sla
```
