# 528 pent

五边形数定理求分拆数 `p(n)`（`p(5..7)=7,11,15`；符号交替 `+ + - - …`）。

- 对标：350_binom → 组合递推版互补（350 为乘法式二项式，此处为广义五边形递推）
- 绕行：负号用 `0 - 1` 表负数（375 同例）；整除皆为精确整除（350 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/528_pent/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/528_pent/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/528_pent/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/528_pent/main.sla -o /tmp/basic_528 && /tmp/basic_528
# 期望输出：7,11,15
```
