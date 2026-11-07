# 526 distinct

互异分拆数 q(n)：每种部至多用一次的 0-1 背包式 DP（`q(5..7)=3,4,5`）。

- 对标：525_practical → 同为划分语义（525 按素因子 Stewart 判定，此处按 DP 计数）
- 绕行：倒序 `while s >= k` 用有符号 int（160 同例，避 `len()` 无符号回绕坑）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/526_distinct/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/526_distinct/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/526_distinct/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/526_distinct/main.sla -o /tmp/basic_526 && /tmp/basic_526
# 期望输出：3,4,5
```
