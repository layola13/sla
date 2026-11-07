# 687 stir1_row7

Stirling 一类第 7 行 `720,1764,1624,735,175,21,1`（行和 `5040=7!`；循环求和）。

- 对标：686_stir17 → 单格版互补（同式行复算；行和 main 直打）
- 绕行：行和 `720+1764+1624+735+175+21+1=5040` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/687_stir1_row7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/687_stir1_row7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/687_stir1_row7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/687_stir1_row7/main.sla -o /tmp/basic_687 && /tmp/basic_687
# 期望输出：720,1764,5040
```
