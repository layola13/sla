# 111 Struct Args

对标 `tsgosa/demos/310_iface_lit_arg`（`3,7,11`）。

- 原用例另有接口继承（`extends`），SLA 无继承语义，此处取两独立
  结构体 + 字面量/绑定两种传参（与 79/87 的嵌套读写互补）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/111_struct_args/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/111_struct_args/main.sla
```
