# 358 prime sum

区间素数和（`<=20 → 77`；281 的素数版）。

- 对标：281 prime_sum 求和 → 试除判定累加
- 绕行：无（纯 `int` 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/358_prime_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/358_prime_sum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/358_prime_sum/main.sla -o /tmp/basic_358 && /tmp/basic_358
# 期望输出：77,17
```
