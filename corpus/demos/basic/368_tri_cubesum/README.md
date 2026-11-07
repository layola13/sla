# 368 tri cubesum

立方和三向对峙（循环 vs 公式 vs 逆序；365 的立方版）。

- 对标：365 tri_sqsum → `(n(n+1)/2)²` 闭式锁定（`5 → 225` 三向）
- 绕行：无（逆序用有符号 `int`，`i >= 1` 到 0 即停）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/368_tri_cubesum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/368_tri_cubesum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/368_tri_cubesum/main.sla -o /tmp/basic_368 && /tmp/basic_368
# 期望输出：225,225,225
```
