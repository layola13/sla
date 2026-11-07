# 174 Clamp

对标 `tsgosa/demos/1263_clamp`（`5,0,10`）。

- 与 134 的 max/min 双函数互补：此处为三参数单函数夹逼。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/174_clamp/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/174_clamp/main.sla
```
