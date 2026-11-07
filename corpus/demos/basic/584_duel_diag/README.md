# 584 duel_diag

次对角对峙：`c(n,n-1) == n·(n-1)/2`（`n=4`：`6` 对 `6`；`n=5`：`10` 对 `10`；`n=6`：`15` 对 `15`）。

- 对标：576_stirling1 → 单实现见源（对换计数解释：单个对换选二元）
- 绕行：`n·(n-1)` 恒偶，除二精确整除（350 同例精神）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/584_duel_diag/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/584_duel_diag/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/584_duel_diag/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/584_duel_diag/main.sla -o /tmp/basic_584 && /tmp/basic_584
# 期望输出：6,6,10,10,15,15
```
