# 496 contfrac

连分数展开（`415/93=[4;2,6,7]`；476 的欧几里得版）。

- 对标：476 egypt → 除法取商迭代（首/项数/末项三函数）
- 绕行：`d != 0` 守卫（商余同步推进；441 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/496_contfrac/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/496_contfrac/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/496_contfrac/main.sla -o /tmp/basic_496 && /tmp/basic_496
# 期望输出：4,4,7
```
