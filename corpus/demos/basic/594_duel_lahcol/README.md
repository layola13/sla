# 594 duel_lahcol

Lah 首列对峙：`L(n,1) == fact(n)`（`n=4`：`24` 对 `24`；`n=5`：`120` 对 `120`；`n=6`：`720` 对 `720`）。

- 对标：592_lah_col → 单实现见源（286 系对峙体例；583 差一体例同形）
- 绕行：同下标直对（无差一，记 README 与 583 区分）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/594_duel_lahcol/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/594_duel_lahcol/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/594_duel_lahcol/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/594_duel_lahcol/main.sla -o /tmp/basic_594 && /tmp/basic_594
# 期望输出：24,24,120,120,720,720
```
