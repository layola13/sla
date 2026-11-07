# 807 eulb_row6

Eulerian-B 第 6 行 `1,722,10543,23548,10543,722,1`（行和 `46080=2^6·6!`；循环求和）。

- 对标：806_eulb6 → 单格版互补（同式行复算；行和 main 直打）
- 绕行：行和七项手算复核（探针三线绿）；`B(6,0)=1` 左端点守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/807_eulb_row6/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/807_eulb_row6/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/807_eulb_row6/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/807_eulb_row6/main.sla -o /tmp/basic_807 && /tmp/basic_807
# 期望输出：722,10543,46080
```
