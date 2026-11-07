# 660 duel_fine_high

Fine 高位对峙：短递推 vs 桥序（`n=5：18 对 18`；`n=6：57 对 57`）。

- 对标：658_fine_high → 高位源互补（659 对峙体例上移；双边同值锁定）
- 绕行：`C(6)-F(5)=132-18=114`，半之 `57`（差偶数恒成立，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/660_duel_fine_high/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/660_duel_fine_high/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/660_duel_fine_high/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/660_duel_fine_high/main.sla -o /tmp/basic_660 && /tmp/basic_660
# 期望输出：18,18,57,57
```
