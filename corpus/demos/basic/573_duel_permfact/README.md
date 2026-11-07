# 573 duel_permfact

全排列对峙：`P(n,n) == fact(n)`（`n=5`：`120` 对 `120`；`n=6`：`720` 对 `720`）。

- 对标：572_perm → 单实现见源（286 系对峙体例；125 阶乘版互补）
- 绕行：降乘与升乘双序同值（572 降序，此处 `fact` 升序，记 README）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/573_duel_permfact/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/573_duel_permfact/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/573_duel_permfact/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/573_duel_permfact/main.sla -o /tmp/basic_573 && /tmp/basic_573
# 期望输出：120,120,720,720
```
