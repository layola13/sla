# 577 rising

上升阶乘 `rise(n,x)=x·(x+1)…(x+n-1)`（`rise(4,1)=24`，`rise(3,2)=24`，`rise(5,1)=120`）。

- 对标：572_perm → 下降版互补（此处为上升连乘；579 做下降对峙）
- 绕行：`rising(2,0)=0` 零起端点（首因子为零则全零，记 README）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/577_rising/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/577_rising/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/577_rising/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/577_rising/main.sla -o /tmp/basic_577 && /tmp/basic_577
# 期望输出：24,24,120
```
