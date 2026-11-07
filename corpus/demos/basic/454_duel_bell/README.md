# 454 duel bell

Bell 双实现对峙（Stirling 求和 vs 二项递推；453 的集合版）。

- 对标：453 duel_trib → `B(n+1)=ΣC(n,k)B(k)` 锁定（`52=52`）
- 绕行：`bell_binom(0)=1` 递归基（451 五基复用；小规模安全）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/454_duel_bell/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/454_duel_bell/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/454_duel_bell/main.sla -o /tmp/basic_454 && /tmp/basic_454
# 期望输出：52,52
```
