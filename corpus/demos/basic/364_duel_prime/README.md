# 364 duel prime

素数判定双实现对峙（试除 vs 跳偶轮式；290 的判定版）。

- 对标：290 试除轮式对峙 → 判定一致性锁定（`29 → 1=1`）
- 绕行：无（双实现皆纯 `int` 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/364_duel_prime/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/364_duel_prime/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/364_duel_prime/main.sla -o /tmp/basic_364 && /tmp/basic_364
# 期望输出：1,1,0
```
