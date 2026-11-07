# 823 duel_col29

Stirling 一类第 2 列对峙：建表 vs 递归（`(8,2)：13068 对 13068`；`(9,2)：109584 对 109584`）。

- 对标：822_stir1_col29 → 列源互补（819 对峙体例列移；双边同值锁定）
- 绕行：`rcs(8,2)` 子树小于探针 `rcs(9,2)`（扇出上确界内，无超时风险）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/823_duel_col29/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/823_duel_col29/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/823_duel_col29/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/823_duel_col29/main.sla -o /tmp/basic_823 && /tmp/basic_823
# 期望输出：13068,13068,109584,109584
```
