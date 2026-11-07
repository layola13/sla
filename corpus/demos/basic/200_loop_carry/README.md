# 200 Loop Carry

对标 `tsgosa/demos/1397_loop_var_after`（末轮值 `k=2`）。

- `for in` 循环变量域外不可见，以外层变量带出（与 163 的调用界互补）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/200_loop_carry/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/200_loop_carry/main.sla
```
