# 56 Chain Ops

对标 `tsgosa/demos/165_chain`（链式赋值 `a = b = 5`）。

- 缺口 #12 已修复：链式赋值可直写（parser desugar 为一次性求值的块）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/56_chain_ops/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/56_chain_ops/main.sla
```
