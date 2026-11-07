# 183 Factory Struct

对标 `tsgosa/demos/482_factory_arg`（`42,41`）。

- 函数返回结构体值语义正常（与 #17 的数组返回缺口对照）；
  参数侧见 111。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/183_factory_struct/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/183_factory_struct/main.sla
```
