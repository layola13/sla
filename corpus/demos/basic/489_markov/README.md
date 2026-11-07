# 489 markov

Markov 数判定与计数（`≤30 → 5`；428 的三元版）。

- 对标：428 taxicab → 三重有序枚举（`a≤b≤c` 去重；`(1,2,3) → 0`）
- 绕行：内层 `while` 后不加 `;`（418 同例）；立方量级 30³ 可承受

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/489_markov/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/489_markov/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/489_markov/main.sla -o /tmp/basic_489 && /tmp/basic_489
# 期望输出：1,0,5
```
