# 265 Deep Count

三层区间计数（`2*3*4=24`）。

- 与 146/196（双层）互补，此处首覆三层（224 为结构四层）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/265_deep_count/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/265_deep_count/main.sla
```
