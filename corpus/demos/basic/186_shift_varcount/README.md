# 186 Shift Varcount

对标 `tsgosa/demos/1233_shift_varcount`（`8,2`）。

- 与 119/176 的字面量位移互补：此处计数来自局部绑定
  （局部 `const` 不可用，探针结论见账本）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/186_shift_varcount/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/186_shift_varcount/main.sla
```
