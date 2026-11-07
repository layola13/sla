# 570 euler_row

Eulerian 行和 `Σ_d A(n,d) = n!`（`row3=6`，`row4=24`，`row5=120`）。

- 对标：569_euler → 单格版互补（532 行和体例；571 做阶乘对峙）
- 绕行：上界 `d ≤ n-1` 收窄（`A(n,n)` 恒零区不读，532 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/570_euler_row/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/570_euler_row/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/570_euler_row/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/570_euler_row/main.sla -o /tmp/basic_570 && /tmp/basic_570
# 期望输出：6,24,120
```
