# 686 stir17

Stirling 一类续高位建表 `c(n,k)=c(n-1,k-1)+(n-1)·c(n-1,k)`（上三角守卫，646 范式）。
`c(7,2)=1764`，`c(7,3)=1624`，`c(7,4)=735`。

- 对标：576_stirling1 → 低位建表版互补（此处为 `n=7` 高位行；689 做建表/递归对峙）
- 绕行：`274+6·225=1624` 双项手算复核；`c(6,2)=274` 低位锚守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/686_stir17/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/686_stir17/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/686_stir17/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/686_stir17/main.sla -o /tmp/basic_686 && /tmp/basic_686
# 期望输出：1764,1624,735
```
