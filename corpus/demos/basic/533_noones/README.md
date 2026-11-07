# 533 noones

无 1 部数分拆：最小部 `≥2`（`n=6,7,8 → 4,4,7`；`8` 含 `[2,2,2,2]` 等 7 组）。

- 对标：529_coinchange → 同为完全背包（529 硬币为 `{1,2,5}`，此处为 `{2..n}` 全集）
- 绕行：`dp[0]=1` 空选基准（背包计数通式，526 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/533_noones/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/533_noones/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/533_noones/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/533_noones/main.sla -o /tmp/basic_533 && /tmp/basic_533
# 期望输出：4,4,7
```
