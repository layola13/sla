# 910 duel_bell12

Bell 高位四对峙：二项递推 vs Aitken 三角（`n=11：678570` 双锁，`n=12：4213597` 双锁）。

- 对标：909_bell12 × 908_duel_bell11 → 高位源与对峙体例合流
- 绕行：递推递归 `n=12` 双后端秒级（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/910_duel_bell12/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/910_duel_bell12/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/910_duel_bell12/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/910_duel_bell12/main.sla -o /tmp/basic_910 && /tmp/basic_910
# 期望输出：678570,678570,4213597,4213597
```
