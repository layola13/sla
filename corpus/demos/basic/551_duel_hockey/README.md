# 551 duel_hockey

hockey 对峙：直接对角求和 `==` 查表 `C(n+1,k+1)`（`(2,6)`：`35` 对 `35`；`(3,7)`：`70` 对 `70`）。

- 对标：548_hockey → 单实现见源（286 系对峙体例）
- 绕行：查表侧下标 `n+1` 显式传参（`binom(7,3)` 即 `C(6+1,2+1)`，记 README 防差一）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/551_duel_hockey/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/551_duel_hockey/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/551_duel_hockey/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/551_duel_hockey/main.sla -o /tmp/basic_551 && /tmp/basic_551
# 期望输出：35,35,70,70
```
