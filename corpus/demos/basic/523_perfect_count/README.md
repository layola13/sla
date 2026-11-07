# 523 perfect count

完全数计数（`<=30 → 2`；373 的完全版）。

- 对标：373 prime_count → 等值判定计数（`6/28` 双完全数）
- 绕行：`n < 2` 守卫（`σ(1)=1≠2` 天然为 0，守卫保语义显式）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/523_perfect_count/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/523_perfect_count/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/523_perfect_count/main.sla -o /tmp/basic_523 && /tmp/basic_523
# 期望输出：2,1,1
```
