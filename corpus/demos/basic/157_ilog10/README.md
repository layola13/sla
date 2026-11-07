# 157 Ilog10

对标 `tsgosa/demos/983_math_aux` 的 log10 部（`2,0,3,0`）。

- SLA 尚无 `Math.log10`（`sqrt` 见 117 同类），用循环除数表达；
  `sign` 部见 118，此处只取 log10。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/157_ilog10/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/157_ilog10/main.sla
```
