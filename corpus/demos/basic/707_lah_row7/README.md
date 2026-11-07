# 707 lah_row7

Lah 第 7 行 `5040,15120,12600,4200,630,42,1`（行和 `37633`；循环求和）。

- 对标：706_lah7 → 单格版互补（同式行复算；行和 main 直打）
- 绕行：行和七项手算复核（探针三线绿）；`L(7,7)=1` 对角端点守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/707_lah_row7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/707_lah_row7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/707_lah_row7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/707_lah_row7/main.sla -o /tmp/basic_707 && /tmp/basic_707
# 期望输出：5040,15120,37633
```
