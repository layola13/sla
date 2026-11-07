# 197 Step2 Up

对标 `tsgosa/demos/1279_while_even`（`2+4+6+8=20`）。

- 与 159（步长 `-2` 下行）、61（步长 `-1` 下行）互补，覆盖上行方向。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/197_step2_up/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/197_step2_up/main.sla
```
