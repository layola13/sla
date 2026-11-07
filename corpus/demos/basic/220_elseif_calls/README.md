# 220 Elseif Calls

对标 `tsgosa/demos/1066_elseif_calls`（`k()=2`→`2`）。

- TS 用 `===`（SLA 无该 token），此处用 `==`；与 50（变量判别）互补。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/220_elseif_calls/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/220_elseif_calls/main.sla
```
