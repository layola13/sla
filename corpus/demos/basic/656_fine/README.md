# 656 fine

Fine 数短递推 `F(n)=[2(2n-1)(2F(n-1)+F(n-2))-F(n-1)(n+1)]/[2(n+1)]`（精确整除）。
`F(3)=2`，`F(4)=6`，`F(5)=18`。

- 对标：351_catalan → Catalan 姐妹序列（`C(n)=2F(n)+F(n-1)`；659 做短递推/桥序对峙）
- 绕行：`F(0)=1`、`F(1)=0` 双基守卫；分子整体先乘后除（596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/656_fine/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/656_fine/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/656_fine/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/656_fine/main.sla -o /tmp/basic_656 && /tmp/basic_656
# 期望输出：2,6,18
```
