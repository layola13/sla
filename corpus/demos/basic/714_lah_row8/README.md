# 714 lah_row8

Lah 第 8 行 `40320,141120,141120,58800,11760,1176,56,1`（行和 `394353`；循环求和）。

- 对标：707_lah_row7 → 低位行互补（同式高位行复算；行和恒等再锁）
- 绕行：行和八项手算复核（探针三线绿）；`L(8,8)=1` 对角端点守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/714_lah_row8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/714_lah_row8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/714_lah_row8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/714_lah_row8/main.sla -o /tmp/basic_714 && /tmp/basic_714
# 期望输出：40320,141120,394353
```
