# 18 Fib Sieve

对标 `tsgosa/demos/173_fib_loop` + `128_sieve`。

- `fib`：循环版斐波那契（`fib(10) = 55`）。
- `prime_count`：试除法素数计数（20 以内 8 个：2 3 5 7 11 13 17 19），
  为筛法的可验证简化版（完整筛法的大数组初始化见性能文档 `.repeat + @memset` 一节）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/18_fib_sieve/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/18_fib_sieve/main.sla
```
