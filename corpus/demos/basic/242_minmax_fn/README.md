# 242 Minmax Fn

对标 `tsgosa/demos/1264_minmax_sum` 的函数版（`10`，另断言相等分支 `14`）。

- 134 为分支体版，此处为三元体版 + 组合调用。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/242_minmax_fn/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/242_minmax_fn/main.sla
```
