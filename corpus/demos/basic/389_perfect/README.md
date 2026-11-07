# 389 perfect

完全数判定（`σ(n)=2n`；378 的等值版）。

- 对标：378 abundant → 等值分支（`6 → 1`，`28 → 1`，`12 → 0`）
- 绕行：无（复用 `sigma`；`1/0` 代替布尔）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/389_perfect/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/389_perfect/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/389_perfect/main.sla -o /tmp/basic_389 && /tmp/basic_389
# 期望输出：1,1,0
```
