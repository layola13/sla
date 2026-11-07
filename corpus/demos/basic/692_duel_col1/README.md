# 692 duel_col1

Stirling 一类首列对峙：建表 vs 阶乘直算（`(7,1)：720 对 720`；`(8,1)：5040 对 5040`）。

- 对标：691_stir1_col1 → 首列源互补（230 阶乘源合流；跨族双边锁定）
- 绕行：`c(n,1)=(n-1)!` 恒等天然成立（首列无探索分歧，一遍绿预期）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/692_duel_col1/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/692_duel_col1/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/692_duel_col1/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/692_duel_col1/main.sla -o /tmp/basic_692 && /tmp/basic_692
# 期望输出：720,720,5040,5040
```
