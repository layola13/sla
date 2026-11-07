# 683 eulb_row5

Eulerian-B 第 5 行 `1,237,1682,1682,237,1`（行和 `3840=2^5·5!`；循环求和）。

- 对标：677_eulb_row → 低位行互补（同式高位行复算；行和恒等再锁）
- 绕行：行和 `1+237+1682+1682+237+1=3840` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/683_eulb_row5/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/683_eulb_row5/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/683_eulb_row5/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/683_eulb_row5/main.sla -o /tmp/basic_683 && /tmp/basic_683
# 期望输出：237,1682,3840
```
