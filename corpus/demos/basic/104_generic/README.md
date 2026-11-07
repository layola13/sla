# 104 Generic

对标 `tsgosa/demos/34_generic_fn`（`5, true, 7`）。

- 原用例另有 `id("hi")` 字符串实例，字符串泛型未经验证，此处取 `int + bool`
  两具化（单态化去重见 monomorphizer，探针已验证双后端 + build-exe）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/104_generic/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/104_generic/main.sla
```
