# 485 primroot

最小本原根（`g(7)=3`；397 的阶版）。

- 对标：397 fermat → 阶等于 `φ(p)` 即本原（`g(5)=2`，`g(11)=2`）
- 绕行：`order` 上界 `p`（找不到返 0；素数必有根无碍）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/485_primroot/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/485_primroot/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/485_primroot/main.sla -o /tmp/basic_485 && /tmp/basic_485
# 期望输出：3,2,2
```
