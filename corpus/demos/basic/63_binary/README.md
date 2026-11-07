# 63 Binary Build

对标 `tsgosa/demos/187_binary`（8 轮拼出 `01010101b=85`）。

- 与 `44_bit_count`（拆位计数）互补，此处为组装方向。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/63_binary/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/63_binary/main.sla
```
