# 563 duel_central

中心二项式对峙：Pascal 查表 `==` 乘法式（`n=4`：`70` 对 `70`；`n=5`：`252` 对 `252`）。

- 对标：546_pascal × 557_central → 两源单实现见各源（286 系对峙体例）
- 绕行：查表侧 `(2n,n)` 双参直传（`binom_table(10,5)` 即 `C(2·5,5)`，记 README 防差一）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/563_duel_central/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/563_duel_central/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/563_duel_central/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/563_duel_central/main.sla -o /tmp/basic_563 && /tmp/basic_563
# 期望输出：70,70,252,252
```
