# 868 duel_fub9

有序 Bell 高位对峙：二项递推 vs 加权和（`n=8：545835` 双锁，`n=9：7087261` 双锁）。

- 对标：866_fub9 × 867_fubw9 → 高位源与对峙体例合流（740 体例续高位）
- 绕行：无（双实现互锁，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/868_duel_fub9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/868_duel_fub9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/868_duel_fub9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/868_duel_fub9/main.sla -o /tmp/basic_868 && /tmp/basic_868
# 期望输出：545835,545835,7087261,7087261
```
