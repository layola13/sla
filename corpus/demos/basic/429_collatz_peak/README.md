# 429 collatz peak

冰雹轨迹峰值（`27 → 9232`；236 的峰值版）。

- 对标：236 collatz → `if/else` 单步 + 峰值跟踪（`7 → 52`）
- 绕行：无（`m != 1` 守卫；纯 `int` 迭代）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/429_collatz_peak/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/429_collatz_peak/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/429_collatz_peak/main.sla -o /tmp/basic_429 && /tmp/basic_429
# 期望输出：52,9232
```
