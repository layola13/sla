# 459 cake

蛋糕数（`5 刀 → 26`；458 的三次版）。

- 对标：458 lazy → 三次闭式（`3 → 8`，`0 → 1`）
- 绕行：无（纯 `int` 闭式；分子恒被 6 整除）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/459_cake/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/459_cake/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/459_cake/main.sla -o /tmp/basic_459 && /tmp/basic_459
# 期望输出：26,8,1
```
