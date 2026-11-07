# 524 abundant sum

丰数和（`<=20 → 50`；358 的丰数版）。

- 对标：358 prime_sum → 盈数项累加（`<=12 → 12`，`<=11 → 0`）
- 绕行：无（`σ>2n` 直判；纯 `int` 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/524_abundant_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/524_abundant_sum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/524_abundant_sum/main.sla -o /tmp/basic_524 && /tmp/basic_524
# 期望输出：50,12,0
```
