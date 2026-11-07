# 81 Reduce

对标 `tsgosa/demos/25_reduce`（求和 `10` + 右折求积 `24`）。

- 高阶 `reduce` 用显式循环表达（迭代器协议见 Phase 6，写法见 `39/67/78`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/81_reduce/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/81_reduce/main.sla
```
