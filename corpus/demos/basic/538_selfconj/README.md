# 538 selfconj

自共轭分拆计数 `==` 互异奇部数分拆（奇硬币 0-1 背包：`sc(6)=1`，`sc(8)=2`，`sc(9)=2`）。

- 对标：526_distinct → 全硬币互异版互补（此处限奇硬币 `k = k + 2` 倒序）
- 绕行：倒序 `while s >= k` 有符号 int（526 同例）；`sc(6)` 仅 `[3,2,1]` 一组

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/538_selfconj/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/538_selfconj/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/538_selfconj/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/538_selfconj/main.sla -o /tmp/basic_538 && /tmp/basic_538
# 期望输出：1,2,2
```
