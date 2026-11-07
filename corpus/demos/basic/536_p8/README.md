# 536 p8

全分拆数 `p(n)`：硬币 `{1..n}` 完全背包 DP（`p(8..10)=22,30,42`）。

- 对标：528_pent → 递推版互补（此处走背包 DP；543 做双实现对峙）
- 绕行：纯 int 累加（值域内无溢出，`p(10)=42`，记 README）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/536_p8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/536_p8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/536_p8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/536_p8/main.sla -o /tmp/basic_536 && /tmp/basic_536
# 期望输出：22,30,42
```
