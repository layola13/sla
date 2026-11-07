# 189 Calls Array

对标 `tsgosa/demos/1063_calls_in_array`（`60,30`）。

- 数组字面量元素可为任意求值表达式（调用、算术皆可），
  与字面量数组（08）口径一致。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/189_calls_array/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/189_calls_array/main.sla
```
