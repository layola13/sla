# 464 duel delannoy

Delannoy 双实现对峙（格路递推 vs 二项和；463 的格路版）。

- 对标：463 duel_cake → `ΣC(m,k)C(n,k)2^k` 锁定（`63=63`）
- 绕行：`k ≤ m,n` 双界（短边截断；`m==n` 对称无碍）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/464_duel_delannoy/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/464_duel_delannoy/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/464_duel_delannoy/main.sla -o /tmp/basic_464 && /tmp/basic_464
# 期望输出：63,63
```
