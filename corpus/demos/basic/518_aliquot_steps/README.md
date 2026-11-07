# 518 aliquot steps

真因子链步数（`12 → 7`；378 的链版）。

- 对标：378 abundant → 链式迭代计数（`6 → 0` 自守，`220 → 1` 入环）
- 绕行：自守/二环两守卫（`prev` 记忆；`0-1` 初值避 `0` 歧义）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/518_aliquot_steps/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/518_aliquot_steps/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/518_aliquot_steps/main.sla -o /tmp/basic_518 && /tmp/basic_518
# 期望输出：7,0,1
```
