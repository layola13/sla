# 426 prim pyth

本原勾股数组计数（`斜边≤30 → 5`；338 的计数版）。

- 对标：338 pyth_check → 双层枚举 + 斜边扫描（`gcd(a,b)==1` 判本原）
- 绕行：`if` 在 `while` 体内照常加 `;`（418 同例；禁的只是 `while}` 后 `;`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/426_prim_pyth/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/426_prim_pyth/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/426_prim_pyth/main.sla -o /tmp/basic_426 && /tmp/basic_426
# 期望输出：5,2
```
