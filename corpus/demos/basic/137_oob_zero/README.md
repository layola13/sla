# 137 Oob Zero

对标 `tsgosa/demos/701_top_arr_oob`（`8,0`）。

- 动态下标越界读到 `0`（与 tsgosa 期望一致，双后端 + exe 核对）；
  字面量越界下标（`a[9]`）过 check 但 exe lowering 不支持，
  此处一律用动态下标（合法性前置判断见 123）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/137_oob_zero/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/137_oob_zero/main.sla
```
