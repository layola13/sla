# 231 Minmax Sum

对标 `tsgosa/demos/1264_minmax_sum`（`min(3,7)+max(3,7)=10`，另断言相等分支 `14`）。

- min2/max2 单体见 134，此处覆盖组合调用。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/231_minmax_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/231_minmax_sum/main.sla
```
