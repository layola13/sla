# 694 stir1_row8

Stirling 一类第 8 行 `5040,13068,13132,6769,1960,322,28,1`（行和 `40320=8!`；循环求和）。

- 对标：687_stir1_row7 → 低位行互补（同式高位行复算；行和恒等再锁）
- 绕行：行和八项手算复核（探针三线绿）；`c(8,8)=1` 对角端点守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/694_stir1_row8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/694_stir1_row8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/694_stir1_row8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/694_stir1_row8/main.sla -o /tmp/basic_694 && /tmp/basic_694
# 期望输出：5040,13068,40320
```
