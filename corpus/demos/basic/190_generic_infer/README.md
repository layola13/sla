# 190 Generic Infer

对标 `tsgosa/demos/1075_generic_erased`（`7,true`）。

- 104 覆盖显式具化（`id<int>(5)`），此处为省略具化的推断调用
  （探针已验证双后端；字符串实例仍暂略，见 104）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/190_generic_infer/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/190_generic_infer/main.sla
```
