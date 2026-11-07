# 171 Overflow 64

对标 `tsgosa/demos/1237_mul_overflow`（SLA 口径末项 `2147483649`）。

- SLA 整数 64 位不回绕（TS 32 位末项得 -2147483647）；
  此处按 SLA 实际语义断言（见 128/168 同主题）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/171_overflow_64/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/171_overflow_64/main.sla
```
