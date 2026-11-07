# 712 duel_lahcol

Lah 首列对峙：建表 vs 阶乘直算（`(7,1)：5040 对 5040`；`(8,1)：40320 对 40320`）。

- 对标：711_lah_col1 → 首列源互补（230 阶乘源合流；594 首列对峙体例）
- 绕行：首列恒等天然成立（一遍绿预期；建表首列与直算双边同值）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/712_duel_lahcol/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/712_duel_lahcol/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/712_duel_lahcol/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/712_duel_lahcol/main.sla -o /tmp/basic_712 && /tmp/basic_712
# 期望输出：5040,5040,40320,40320
```
