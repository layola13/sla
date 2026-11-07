# 50 Nested Sign Min Step

对标 `tsgosa/demos/189_nest_if3` + `175_min3` + `199_for_step2`。

- `sign`：嵌套 `if` + `else if`（`200->2, 5->1, -3->-1, 0->0`）。
- `min3`：顺序 `if` 收窄。
- `step_sum(10, 2) = 20`：SLA `for` 无步长语法，用 `while + i = i + step` 表达
  （`for i in 0..n` 只支持步长 1）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/50_nested_sign_min_step/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/50_nested_sign_min_step/main.sla
```
