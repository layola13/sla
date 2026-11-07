# 706 lah7

Lah 续高位建表 `L(n,k)=L(n-1,k-1)+(n+k-1)·L(n-1,k)`（上三角守卫，646 范式）。
`L(7,2)=15120`，`L(7,3)=12600`，`L(7,4)=4200`。

- 对标：586_lah → 低位商式版互补（此处为 `n=7` 高位行；709 做建表/商式对峙）
- 绕行：`1800+9·1200=12600` 双项手算复核；首列阶乘自然导出（`L(2,1)=2`，免预填）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/706_lah7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/706_lah7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/706_lah7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/706_lah7/main.sla -o /tmp/basic_706 && /tmp/basic_706
# 期望输出：15120,12600,4200
```
