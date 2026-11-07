# 391 mersenne

梅森数素性（`p=11 → 2047` 合；356 的指数版）。

- 对标：356 is_prime → `2^p-1` 复用判定（`3/7/31` 素，`2047` 合）
- 绕行：无（`pow2` 循环复用；纯 `int` 试除）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/391_mersenne/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/391_mersenne/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/391_mersenne/main.sla -o /tmp/basic_391 && /tmp/basic_391
# 期望输出：1,1,1,0
```
