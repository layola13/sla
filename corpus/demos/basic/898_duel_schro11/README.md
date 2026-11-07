# 898 duel_schro11

大 Schröder 续高位三对峙：迭代 vs 商式（`n=10：1037718` 双锁，`n=11：5293446` 双锁）。

- 对标：896_schro11 × 793_schro_qseq → 高位源与商式源合流（789 体例续高位）
- 绕行：无（双实现互锁，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/898_duel_schro11/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/898_duel_schro11/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/898_duel_schro11/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/898_duel_schro11/main.sla -o /tmp/basic_898 && /tmp/basic_898
# 期望输出：1037718,1037718,5293446,5293446
```
