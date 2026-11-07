# 245 Formula

对标 `tsgosa/demos/137_tri` / `141_sum_sq` 的公式版（71/27 为循环版：`55/55`）。

- 闭式求和，整除恰好整除（45 口径）；另断言 `tri(0)`/`sqsum(1)` 边界。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/245_formula/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/245_formula/main.sla
```
