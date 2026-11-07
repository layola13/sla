# 428 taxicab

出租车数表示计数（`1729 → 2`；426 的立方版）。

- 对标：426 prim_pyth → 双层枚举立方和（`a≤b` 去重，`4104 → 2`）
- 绕行：`a³<n` 外界（`a==b` 允许，`1000 → 0` 正整数对无解）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/428_taxicab/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/428_taxicab/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/428_taxicab/main.sla -o /tmp/basic_428 && /tmp/basic_428
# 期望输出：2,2,0
```
