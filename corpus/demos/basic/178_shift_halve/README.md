# 178 Shift Halve

对标 `tsgosa/demos/1256_shift_while`（`8,1`）。

- SLA 无 `>>=`（无词法 token，见 119），用 `x = x >> 1` 表达；
  与 170 的左移翻倍方向互补。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/178_shift_halve/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/178_shift_halve/main.sla
```
