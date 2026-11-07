# 806 eulb6

Eulerian-B 续高位 `B(6,1)=722`，`B(6,2)=10543`，`B(6,3)=23548`（同式再上两行）。

- 对标：676_eulb → `n≤5` 版互补（同式再上两行；809 做建表/交错和对峙）
- 绕行：`7·1682+7·1682=23548` 对称双项手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/806_eulb6/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/806_eulb6/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/806_eulb6/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/806_eulb6/main.sla -o /tmp/basic_806 && /tmp/basic_806
# 期望输出：722,10543,23548
```
