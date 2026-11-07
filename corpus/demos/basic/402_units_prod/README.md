# 402 units prod

模单位积（`n=9 → 8`，`n=8 → 1`；381 的乘积版）。

- 对标：381 coprime_sum → 逐项取模乘积（Gauss 定理实例）
- 绕行：无（`r * k % m` 逐项取模防溢出）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/402_units_prod/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/402_units_prod/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/402_units_prod/main.sla -o /tmp/basic_402 && /tmp/basic_402
# 期望输出：8,9,1
```
