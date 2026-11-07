# 210 Sw Field

对标 `tsgosa/demos/939_sw_member`（`v=2`→`2`）。

- 结构体字段路径作 `switch` 判别式；与 151（调用结果）、
  207（三元式）互补。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/210_sw_field/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/210_sw_field/main.sla
```
