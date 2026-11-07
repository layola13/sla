# 840 duel_lah910

Lah 续高位二对峙：建表 vs 商式（`(9,4)=846720` 双锁，`(8,4)=58800` 双锁）。

- 对标：836_lah9 × 839_duel_lah9 → 内格源与对峙体例合流（845 做三向）
- 绕行：`56·362880/24=846720` 商式手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/840_duel_lah910/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/840_duel_lah910/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/840_duel_lah910/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/840_duel_lah910/main.sla -o /tmp/basic_840 && /tmp/basic_840
# 期望输出：846720,846720,58800,58800
```
