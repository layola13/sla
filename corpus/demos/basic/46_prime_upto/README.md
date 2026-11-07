# 46 Prime Upto

对标 `tsgosa/demos/183_prime_upto`。

与 `18_fib_sieve`（试除计数到 20）互补：此处 `for + break` 剪枝（`i*i>n` 跳出内层），
`30` 以内 `10` 个素数。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/46_prime_upto/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/46_prime_upto/main.sla
```
