# 515 pow2check

二幂判定（`8 → 1`；514 的位技巧版）。

- 对标：514 parity → `n&(n-1)` 经典式（`1 → 1`，`0 → 0` 守卫）
- 绕行：`n <= 0` 先排除（`0&(0-1)` 语义坑）；`&` 加括号

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/515_pow2check/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/515_pow2check/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/515_pow2check/main.sla -o /tmp/basic_515 && /tmp/basic_515
# 期望输出：1,0,1,0
```
