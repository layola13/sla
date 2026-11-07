# 697 stir2_row7

Stirling 二类第 7 行 `1,63,301,350,140,21,1`（行和 `877=Bell(7)`；循环求和）。

- 对标：696_stir27 → 单格版互补（同式行复算；行和 main 直打）
- 绕行：行和七项手算复核（探针三线绿）；`S(7,7)=1` 对角端点守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/697_stir2_row7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/697_stir2_row7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/697_stir2_row7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/697_stir2_row7/main.sla -o /tmp/basic_697 && /tmp/basic_697
# 期望输出：63,301,877
```
