# 54 Neg Index

对标 `tsgosa/demos/162_at_neg` / `169_neg_idx`（`at(-1)` 取尾）。

- SLA 暂无 `at(-1)` 负索引，用 `a[len(a) - 1]` 表达，语义等价。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/54_neg_index/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/54_neg_index/main.sla
```
