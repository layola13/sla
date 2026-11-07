# 164 Closure Array

对标 `tsgosa/demos/1071_closure_arr`（`8,4`）。

- 闭包捕获数组 + 参数索引（与 20 的标量捕获互补；闭包体仅表达式，
  块体/赋值捕获不可解析，探针结论见账本）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/164_closure_array/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/164_closure_array/main.sla
```
