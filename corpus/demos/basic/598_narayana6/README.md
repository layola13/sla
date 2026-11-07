# 598 narayana6

Narayana 高位行 `n=6`：`1,15,50,50,15,1`（和 `132=C(6)`；行和循环求和）。

- 对标：597_narayana_row → 低位版互补（同式高位复算；`300/6=50` 除法核对）
- 绕行：上界 `k ≤ n-1` 收窄（`N(n,n)` 越界区不读，532 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/598_narayana6/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/598_narayana6/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/598_narayana6/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/598_narayana6/main.sla -o /tmp/basic_598 && /tmp/basic_598
# 期望输出：15,50,50,132
```
