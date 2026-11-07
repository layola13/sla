# 411 duel dsum

数位和双实现对峙（循环 vs 递归；364 的数位版）。

- 对标：364 duel_prime → 递归 `n%10 + f(n/10)` 一致性锁定（`12345 → 15=15`）
- 绕行：递归基 `n < 10` 直接返回（07_fact 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/411_duel_dsum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/411_duel_dsum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/411_duel_dsum/main.sla -o /tmp/basic_411 && /tmp/basic_411
# 期望输出：15,15
```
