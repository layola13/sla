# 398 wilson

威尔逊定理测试（`(p-1)! mod p`；397 的阶乘版）。

- 对标：397 fermat → 阶乘取模判定（`5/7 → 1`，`6/4 → 0`）
- 绕行：小 p 避阶乘溢出（逐项取模；纯 `int` 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/398_wilson/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/398_wilson/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/398_wilson/main.sla -o /tmp/basic_398 && /tmp/basic_398
# 期望输出：1,1,0,0
```
