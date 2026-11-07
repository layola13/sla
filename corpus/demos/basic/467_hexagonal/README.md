# 467 hexagonal

六边形数（`H(5)=45`；466 的六边版）。

- 对标：466 pentagonal → `n(2n-1)` 闭式（`H(4)=28`）
- 绕行：无（纯 `int` 闭式；恒整）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/467_hexagonal/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/467_hexagonal/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/467_hexagonal/main.sla -o /tmp/basic_467 && /tmp/basic_467
# 期望输出：45,28,1
```
