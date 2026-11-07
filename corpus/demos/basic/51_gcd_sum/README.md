# 51 Gcd Sum

对标 `tsgosa/demos/159_gcd_sum`（`gcd(i, 12)`，`i=1..10`，累加得 `27`）。

- `gcd` 用 `while` 循环版（与 `17/21` 的递归版互补）。
- `for i in 1..11` 表达 `1..=10` 闭区间。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/51_gcd_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/51_gcd_sum/main.sla
```
