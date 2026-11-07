# 900 duel_schro12

大 Schröder 续高位四对峙：迭代 vs 商式（`n=11：5293446` 双锁，`n=12：27297738` 双锁）。

- 对标：899_schro12 × 898_duel_schro11 → 高位源与对峙体例合流（790 体例续高位）
- 绕行：无（双实现互锁，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/900_duel_schro12/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/900_duel_schro12/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/900_duel_schro12/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/900_duel_schro12/main.sla -o /tmp/basic_900 && /tmp/basic_900
# 期望输出：5293446,5293446,27297738,27297738
```
