# 818 duel_col19

Stirling 一类首列续对峙：建表 vs 阶乘（`(8,1)：5040 对 5040`；`(9,1)：40320 对 40320`）。

- 对标：691_stir1_col1 → `n≤8` 版互补（692 体例续高位；跨族双边锁定）
- 绕行：首列恒等天然成立（一遍绿预期；建表首列与直算双边同值）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/818_duel_col19/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/818_duel_col19/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/818_duel_col19/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/818_duel_col19/main.sla -o /tmp/basic_818 && /tmp/basic_818
# 期望输出：5040,5040,40320,40320
```
