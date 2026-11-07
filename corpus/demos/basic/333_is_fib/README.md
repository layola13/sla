# 333 Is Fib

fib 成员判定（`8→1`，`9→0`，`13→1`）。

- 意对标 `tsgosa/demos/173_fib_loop` 的成员版：`5n²±4` 完全平方即 fib 数（与 325 同构）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/333_is_fib/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/333_is_fib/main.sla
```
