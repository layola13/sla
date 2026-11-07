# 667 eul_row

Eulerian 第 5 行 `1,26,66,26,1`（行和 `120=5!`；循环求和）。

- 对标：666_eulerian → 单格版互补（同式行复算；行和 main 直打）
- 绕行：行和恒等 `sum(E(5,m))=5!` 双锁（674 做阶乘桥；`E(5,0)=1` 端点守卫）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/667_eul_row/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/667_eul_row/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/667_eul_row/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/667_eul_row/main.sla -o /tmp/basic_667 && /tmp/basic_667
# 期望输出：26,66,120
```
