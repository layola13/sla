# 373 prime count

区间素数计数（`<=50 → 15`；358 的计数版）。

- 对标：358 prime_sum → 计数累加（`<=30 → 10`，`<=10 → 4`）
- 绕行：无（纯 `int` 循环复用 `is_prime`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/373_prime_count/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/373_prime_count/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/373_prime_count/main.sla -o /tmp/basic_373 && /tmp/basic_373
# 期望输出：15,10,4
```
