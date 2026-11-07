# 870 duel_fubp9

有序 Bell 表商对峙：表版 vs 商版（`n=8：545835` 双锁，`n=9：7087261` 双锁）。

- 对标：872_fubp9 × 866_fub9 → 表源与商源合流（743 体例续高位）
- 绕行：表首列逐行置 1（835 探针纠偏复用；探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/870_duel_fubp9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/870_duel_fubp9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/870_duel_fubp9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/870_duel_fubp9/main.sla -o /tmp/basic_870 && /tmp/basic_870
# 期望输出：545835,545835,7087261,7087261
```
