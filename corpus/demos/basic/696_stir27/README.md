# 696 stir27

Stirling 二类续高位建表 `S(n,k)=S(n-1,k-1)+k·S(n-1,k)`（上三角守卫，646 范式）。
`S(7,2)=63`，`S(7,3)=301`，`S(7,4)=350`。

- 对标：531_stirling2 → 低位建表版互补（此处为 `n=7` 高位行；699 做建表/递归对峙）
- 绕行：`31+3·90=301` 双项手算复核；`S(6,3)=90` 低位锚守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/696_stir27/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/696_stir27/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/696_stir27/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/696_stir27/main.sla -o /tmp/basic_696 && /tmp/basic_696
# 期望输出：63,301,350
```
