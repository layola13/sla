# 177 Shift Chain

对标 `tsgosa/demos/1239_shift_chain` + `1247_shift_compare`
（SLA 口径 `4294967296,1,0`）。

- 64 位语义再确认：连续位移不回绕，`(1 << 31) < 0` 为假
  （TS 三项皆反）；见 128/168/171 同主题。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/177_shift_chain/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/177_shift_chain/main.sla
```
