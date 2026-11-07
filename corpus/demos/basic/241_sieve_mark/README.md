# 241 Sieve Mark

对标 `tsgosa/demos/128_sieve` 的真筛版（18 为试除计数版：30 以内 `10` 个）。

- 标记数组 + 倍数清零；`marks[29]=1`/`marks[30]=0` 锁定边界。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/241_sieve_mark/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/241_sieve_mark/main.sla
```
