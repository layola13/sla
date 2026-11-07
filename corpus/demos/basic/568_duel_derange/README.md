# 568 duel_derange

错排对峙：双项递推 `==` 容斥和式（`n=5`：`44` 对 `44`；`n=6`：`265` 对 `265`）。

- 对标：566_derange × 567_derange_sum → 两源单实现见各源（286 系对峙体例；567 修正版直用）
- 绕行：`!6=265` 大值双锁（递推与容斥独立算出同值）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/568_duel_derange/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/568_duel_derange/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/568_duel_derange/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/568_duel_derange/main.sla -o /tmp/basic_568 && /tmp/basic_568
# 期望输出：44,44,265,265
```
