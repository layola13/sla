# 42 Every Positive

对标 `tsgosa/demos/127_sorted`（`every(x > 0)`）。

与 `20_closures` 的 `vec_every_gt` 互补：此处为定长数组版 + 早返回 `false`。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/42_every_positive/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/42_every_positive/main.sla
```
