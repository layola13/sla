# 159 While Dec2

对标 `tsgosa/demos/1050_while_dec`（`9`，终值 `-1`）。

- 与 158 互补（初值/步长不同：5+3+1；158 为 10+7+4+1）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/159_while_dec2/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/159_while_dec2/main.sla
```
