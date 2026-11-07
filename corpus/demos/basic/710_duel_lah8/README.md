# 710 duel_lah8

Lah 高位对峙：建表 vs 商式（`(8,3)：141120 对 141120`；`(8,4)：58800 对 58800`）。

- 对标：708_lah8 → 高位源互补（709 对峙体例上移；双边同值锁定）
- 绕行：`21·40320/6=141120` 大值先乘后除精确（中间积 `846720` 值域内，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/710_duel_lah8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/710_duel_lah8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/710_duel_lah8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/710_duel_lah8/main.sla -o /tmp/basic_710 && /tmp/basic_710
# 期望输出：141120,141120,58800,58800
```
