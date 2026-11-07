# 527 oddpart

奇部数分拆 p_odd(n)：只用 `{1,3,5}` 奇数部的完全背包 DP（`p_odd(5..7)=3,4,5`）。

- 对标：526_distinct → 互异版互补（Euler 定理 `q(n)==p_odd(n)`，见 534 对峙）
- 绕行：奇数硬币步进 `k = k + 2`（无 `>>`/`^` 算子，记 README）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/527_oddpart/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/527_oddpart/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/527_oddpart/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/527_oddpart/main.sla -o /tmp/basic_527 && /tmp/basic_527
# 期望输出：3,4,5
```
