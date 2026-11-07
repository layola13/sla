# 172 Bit Test

对标 `tsgosa/demos/1248_bit_condition` + `1249_bit_ternary`（`1,0,10,20`）。

- 位运算结果须显式比零（`if (x & 1)` 按严格布尔拒识，见 129）；
  与 76 的纯位运算互补，此处主覆盖“位→布尔”判定。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/172_bit_test/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/172_bit_test/main.sla
```
