# 208 Void Fn

对标 `tsgosa/demos/915_void_call`（打印 `9`）。

- 首个无返回值函数 demo：`void` 算子不可解析，直接以语句调用；
  `@test` 内验证不 trap（副作用输出以 build-exe 核对）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/208_void_fn/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/208_void_fn/main.sla
```
