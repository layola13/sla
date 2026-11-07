# 05 For Sum

对标 `tsgosa/demos/05_for_sum`。

`for i in 0..limit` 为 SLA 的整数区间循环（非 `for in` 迭代器协议，
后者是 roadmap Phase 6 的未完成项，此处不受影响）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/05_for_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/05_for_sum/main.sla
```
