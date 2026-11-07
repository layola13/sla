# 535 coin_duel

换零钱对峙：硬币 `{1,3,4}` 凑 `6`（DP 解 `4`，暴力三重循环解 `4`）。

- 对标：529_coinchange → `{1,2,5}` 版互补（此处 `{1,3,4}` 加暴力枚举对峙，286 系体例）
- 绕行：暴力上界用乘式 `b * 3 <= n` 剪枝（小规模一次绿；三重循环体皆表达式）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/535_coin_duel/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/535_coin_duel/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/535_coin_duel/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/535_coin_duel/main.sla -o /tmp/basic_535 && /tmp/basic_535
# 期望输出：4,4
```
