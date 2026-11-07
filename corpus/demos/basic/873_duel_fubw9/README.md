# 873 duel_fubw9

有序 Bell 表加对峙：表版 vs 加权和（`n=9：7087261` 双锁，`n=8：545835` 双锁）。

- 对标：872_fubp9 × 867_fubw9 → 表源与加权源合流（743 体例续高位）
- 绕行：无（双实现互锁，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/873_duel_fubw9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/873_duel_fubw9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/873_duel_fubw9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/873_duel_fubw9/main.sla -o /tmp/basic_873 && /tmp/basic_873
# 期望输出：7087261,7087261,545835,545835
```
