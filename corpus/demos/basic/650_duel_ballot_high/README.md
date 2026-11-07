# 650 duel_ballot_high

Ballot 高位对峙：建表 vs 商式（`(5,3)：28 对 28`；`(5,5)：42 对 42`）。

- 对标：648_ballot_high → 高位源互补（649 对峙体例上移；双边同值锁定）
- 绕行：`3·56/6=28`、`1·252/6=42` 先乘后除精确（596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/650_duel_ballot_high/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/650_duel_ballot_high/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/650_duel_ballot_high/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/650_duel_ballot_high/main.sla -o /tmp/basic_650 && /tmp/basic_650
# 期望输出：28,28,42,42
```
