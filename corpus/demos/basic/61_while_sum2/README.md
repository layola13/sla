# 61 While Sum2

对标 `tsgosa/demos/198_while_sum2`（`10+9+…+1=55`）。

- 与 `04/05`（正序累加）互补，此处用递减 `while`（风格见 `40_do_sum`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/61_while_sum2/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/61_while_sum2/main.sla
```
