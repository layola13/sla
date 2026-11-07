# 466 pentagonal

五边形数（`P(5)=35`；137 的高边版）。

- 对标：137 tri → `n(3n-1)/2` 闭式（`P(4)=22`）
- 绕行：无（纯 `int` 闭式；分子恒偶）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/466_pentagonal/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/466_pentagonal/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/466_pentagonal/main.sla -o /tmp/basic_466 && /tmp/basic_466
# 期望输出：35,22,1
```
