# 550 duel_row

行和对峙：表求和 `Σ_k C(n,k) ==` 闭式 `2^n`（`n=8`：`256` 对 `256`；`n=10`：`1024` 对 `1024`）。

- 对标：547_rowsum → 单实现见源（此处加 `*2` 循环闭式，286 系对峙体例）
- 绕行：`pow2` 纯循环（无 `**`/`<<` 算子，530 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/550_duel_row/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/550_duel_row/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/550_duel_row/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/550_duel_row/main.sla -o /tmp/basic_550 && /tmp/basic_550
# 期望输出：256,256,1024,1024
```
