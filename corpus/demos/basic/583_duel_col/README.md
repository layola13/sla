# 583 duel_col

首列对峙：`c(n,1) == fact(n-1)`（`n=4`：`6` 对 `6`；`n=5`：`24` 对 `24`；`n=6`：`120` 对 `120`）。

- 对标：582_col → 单实现见源（286 系对峙体例；下标差一显式写 `fact(3/4/5)`）
- 绕行：差一 constellation 显式化（551 同例教训，记 README）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/583_duel_col/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/583_duel_col/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/583_duel_col/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/583_duel_col/main.sla -o /tmp/basic_583 && /tmp/basic_583
# 期望输出：6,6,24,24,120,120
```
