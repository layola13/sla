# 497 convergent

收敛子（二阶 `58/13`；496 的渐近版）。

- 对标：496 contfrac → `p/q` 双递推（零阶 `4/1` 另断言）
- 绕行：分子分母双函数（477 同例）；`p₋₂/q₋₂` 初值对

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/497_convergent/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/497_convergent/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/497_convergent/main.sla -o /tmp/basic_497 && /tmp/basic_497
# 期望输出：58,13,4,1
```
