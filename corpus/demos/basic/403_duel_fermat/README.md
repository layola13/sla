# 403 duel fermat

费马测试双实现对峙（连乘 vs 平方乘；355 的费马版）。

- 对标：355 tri_powmod → 底数 2 锁定（`7 → 1=1`，`341` 双边同误判）
- 绕行：无（双实现皆纯 `int` 循环；伪素数属数论事实）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/403_duel_fermat/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/403_duel_fermat/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/403_duel_fermat/main.sla -o /tmp/basic_403 && /tmp/basic_403
# 期望输出：1,1,1
```
