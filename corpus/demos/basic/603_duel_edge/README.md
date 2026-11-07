# 603 duel_edge

边值对峙：`N(n,1) ==` 三角数 `n·(n-1)/2`（`n=4`：`6` 对 `6`；`n=5`：`10` 对 `10`；`n=6`：`15` 对 `15`）。

- 对标：602_narayana_edge → 单实现见源（286 系对峙体例；584 同形）
- 绕行：`n·(n-1)` 恒偶除二精确（584 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/603_duel_edge/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/603_duel_edge/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/603_duel_edge/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/603_duel_edge/main.sla -o /tmp/basic_603 && /tmp/basic_603
# 期望输出：6,6,10,10,15,15
```
