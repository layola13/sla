# 492 tri markov

Markov 三向（判定/计数/Vieta 上行；489 的三向版）。

- 对标：489 markov → 上行 `3bc-a` 第三角（`(1,2,5) → 29` 且回验成组）
- 绕行：内层 `while` 后不加 `;`（489 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/492_tri_markov/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/492_tri_markov/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/492_tri_markov/main.sla -o /tmp/basic_492 && /tmp/basic_492
# 期望输出：1,5,29
```
