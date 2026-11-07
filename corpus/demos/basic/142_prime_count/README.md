# 142 Prime Count

对标 `tsgosa/demos/183_prime_upto`（30 以内 10 个）。

- 与 18 的筛法互补：此处为单数试除 + 区间计数（`i*i <= n` 剪枝）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/142_prime_count/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/142_prime_count/main.sla
```
