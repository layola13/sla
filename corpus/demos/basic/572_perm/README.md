# 572 perm

排列数 `P(n,k)=n·(n-1)…(n-k+1)`（`P(5,2)=20`，`P(6,3)=120`，`P(10,2)=90`）。

- 对标：350_binom → 除法版互补（此处为无除连乘；567 下降阶乘同形，573 做全排列对峙）
- 绕行：`P(n,0)=1` 空积端点（557 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/572_perm/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/572_perm/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/572_perm/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/572_perm/main.sla -o /tmp/basic_572 && /tmp/basic_572
# 期望输出：20,120,90
```
