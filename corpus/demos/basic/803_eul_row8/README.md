# 803 eul_row8

Eulerian 第 8 行 `1,247,4293,15619,15619,4293,247,1`（行和 `40320=8!`；循环求和）。

- 对标：797_eul_row7 → 低位行互补（同式高位行复算；行和恒等再锁）
- 绕行：行和八项手算复核（探针三线绿）；`E(8,0)=1` 左端点守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/803_eul_row8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/803_eul_row8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/803_eul_row8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/803_eul_row8/main.sla -o /tmp/basic_803 && /tmp/basic_803
# 期望输出：247,4293,40320
```
