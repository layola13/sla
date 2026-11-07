# 621 duel_central

Delannoy 中央对峙：建表 vs 直和（`(4,4)：321 对 321`；`(5,5)：1683 对 1683`）。

- 对标：620_delannoy_central → 中央源互补（612 对峙体例再上移；双边同值锁定）
- 绕行：中央值双边独立复算（建表拍扁与直和六项式异构，625 做三向收束）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/621_duel_central/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/621_duel_central/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/621_duel_central/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/621_duel_central/main.sla -o /tmp/basic_621 && /tmp/basic_621
# 期望输出：321,321,1683,1683
```
