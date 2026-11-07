# 797 eul_row7

Eulerian 第 7 行 `1,120,1191,2416,1191,120,1`（行和 `5040=7!`；循环求和）。

- 对标：796_eul7 → 单格版互补（同式行复算；行和 main 直打）
- 绕行：行和七项手算复核（探针三线绿）；`E(7,0)=1` 左端点守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/797_eul_row7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/797_eul_row7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/797_eul_row7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/797_eul_row7/main.sla -o /tmp/basic_797 && /tmp/basic_797
# 期望输出：120,1191,5040
```
