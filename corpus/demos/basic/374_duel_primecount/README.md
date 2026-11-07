# 374 duel primecount

素数计数双实现对峙（逐试 vs 跳偶；364 的计数版）。

- 对标：364 duel_prime → 计数一致性锁定（`<=20 → 8=8`）
- 绕行：无（双实现皆纯 `int` 循环；内层试除复位标志位）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/374_duel_primecount/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/374_duel_primecount/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/374_duel_primecount/main.sla -o /tmp/basic_374 && /tmp/basic_374
# 期望输出：8,8
```
