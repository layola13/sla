# 260 Bsearch Rec

对标 `tsgosa/demos/101_bsearch` 的递归版（17 为循环版：命中 `3`，未中 `-1`，另断言端点 `0/4`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/260_bsearch_rec/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/260_bsearch_rec/main.sla
```
