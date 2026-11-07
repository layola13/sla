# 908 duel_bell11

Bell 高位三对峙：二项递推 vs Aitken 三角（`n=10：115975` 双锁，`n=11：678570` 双锁）。

- 对标：906_bell11 × 911_belltri11 → 高位源与对峙体例合流（780 体例续高位）
- 绕行：无（双实现互锁，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/908_duel_bell11/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/908_duel_bell11/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/908_duel_bell11/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/908_duel_bell11/main.sla -o /tmp/basic_908 && /tmp/basic_908
# 期望输出：115975,115975,678570,678570
```
