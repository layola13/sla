# 128 Shift Big

对标 `tsgosa/demos/452_shift_bigcount`（SLA 口径 `0,8589934592,-1`）。

- SLA 位移为 64 位无掩码语义，与 TS 的 mod-32 语义（`4,2,-4`）不同，
  此处按实际语义断言；`>>>` / `>>=` / `<<=` 均无词法 token（见 119）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/128_shift_big/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/128_shift_big/main.sla
```
