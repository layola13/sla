# 740 duel_fub8

有序 Bell 高位对峙：二项递推 vs 加权和（`n=7：47293 对 47293`；`n=8：545835 对 545835`）。

- 对标：738_fub8 → 高位源互补（739 对峙体例上移；双边同值锁定）
- 绕行：`F(8)` 加权八项手算复核（探针三线绿；递归深度止 `8`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/740_duel_fub8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/740_duel_fub8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/740_duel_fub8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/740_duel_fub8/main.sla -o /tmp/basic_740 && /tmp/basic_740
# 期望输出：47293,47293,545835,545835
```
