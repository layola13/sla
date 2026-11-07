# 582 col

第一类首列 `c(n,1)=(n-1)!`（`c(4,1)=6`，`c(5,1)=24`，`c(6,1)=120`）。

- 对标：576_stirling1 → 全表版互补（此处首列单序列；544 精确版体例，583 做对峙）
- 绕行：长轮换计数解释（`n` 元轮换 `(n-1)!` 个，记 README）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/582_col/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/582_col/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/582_col/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/582_col/main.sla -o /tmp/basic_582 && /tmp/basic_582
# 期望输出：6,24,120
```
