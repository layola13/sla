# 469 octagonal

八边形数（`O(5)=65`；468 的八边版）。

- 对标：468 heptagonal → `n(3n-2)` 闭式（`O(4)=40`）
- 绕行：无（纯 `int` 闭式；恒整）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/469_octagonal/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/469_octagonal/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/469_octagonal/main.sla -o /tmp/basic_469 && /tmp/basic_469
# 期望输出：65,40,1
```
