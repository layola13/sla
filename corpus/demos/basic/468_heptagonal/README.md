# 468 heptagonal

七边形数（`Hep(5)=55`；467 的七边版）。

- 对标：467 hexagonal → `n(5n-3)/2` 闭式（`Hep(4)=34`）
- 绕行：无（纯 `int` 闭式；分子恒偶）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/468_heptagonal/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/468_heptagonal/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/468_heptagonal/main.sla -o /tmp/basic_468 && /tmp/basic_468
# 期望输出：55,34,1
```
