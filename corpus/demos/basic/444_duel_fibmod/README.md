# 444 duel fibmod

模斐波那契双实现对峙（直迭代 vs Pisano 约化；364 的周期版）。

- 对标：364 duel_prime → 周期约化锁定（`(100,7) → 3=3`，周期 16）
- 绕行：周期上界 `6m`（Pisano 周期必落界内；返回 0 判缺界，7 无碍）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/444_duel_fibmod/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/444_duel_fibmod/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/444_duel_fibmod/main.sla -o /tmp/basic_444 && /tmp/basic_444
# 期望输出：3,3,16
```
