# 118 Sign Imul

对标 `tsgosa/demos/351_math_small`（`sign` 部：`-1,0,1`；`imul` 即乘法 `42`）。

- `abs` 见 100，此处主覆盖 `sign`（分支表达）与 `imul`（即普通乘法）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/118_sign_imul/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/118_sign_imul/main.sla
```
