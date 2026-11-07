# 590 duel_lah

Lah 对峙：商式 `==` 递推（`(4,2)`：`36` 对 `36`；`(5,3)`：`120` 对 `120`）。

- 对标：586_lah × 589_lah_rec → 两源单实现见各源（286 系对峙体例）
- 绕行：双表各自独立（534 同例）；商除链与递推权和式互锁

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/590_duel_lah/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/590_duel_lah/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/590_duel_lah/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/590_duel_lah/main.sla -o /tmp/basic_590 && /tmp/basic_590
# 期望输出：36,36,120,120
```
