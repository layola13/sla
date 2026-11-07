# 17 Bsearch Gcd

对标 `tsgosa/demos/101_bsearch` + `119_gcd3`。

- `bsearch`：有序定长数组二分查找，命中回下标，未命中回 `-1`。
- `gcd`：辗转相除（`while y != 0`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/17_bsearch_gcd/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/17_bsearch_gcd/main.sla
```
