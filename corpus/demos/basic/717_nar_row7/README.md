# 717 nar_row7

Narayana 第 7 行 `1,21,105,175,105,21,1`（行和 `429=Catalan(7)`；循环求和）。

- 对标：716_nar7 → 单格版互补（同式行复算；行和 main 直打）
- 绕行：行和七项手算复核（探针三线绿）；`N(7,0)=1` 左端点守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/717_nar_row7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/717_nar_row7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/717_nar_row7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/717_nar_row7/main.sla -o /tmp/basic_717 && /tmp/basic_717
# 期望输出：21,105,429
```
