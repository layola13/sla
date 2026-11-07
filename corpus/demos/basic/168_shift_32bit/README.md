# 168 Shift 32bit

对标 `tsgosa/demos/1228_shift_sign` 的 `<<`/`>>` 部（`2147483648,-2`）。

- 32 位边界亦为 64 位语义：`1 << 31` 得正数（TS 得 -2147483648）；
  `-8 >> 2` 算术右移得 -2（与 TS 一致）；`>>>` 不可用。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/168_shift_32bit/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/168_shift_32bit/main.sla
```
