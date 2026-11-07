# 392 fact prime exp

阶乘素指数 Legendre（`v2(10!)=8`；349 的素数版）。

- 对标：349 fact_zero → `n/p + n/p² + …` 通式（`v5` 即 349 口径）
- 绕行：无（`d <= n` 乘法步进；纯 `int` 除法）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/392_fact_prime_exp/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/392_fact_prime_exp/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/392_fact_prime_exp/main.sla -o /tmp/basic_392 && /tmp/basic_392
# 期望输出：8,4,2
```
