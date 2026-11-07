# 350 binom

二项式系数（乘法式逐项整除；137 公式系的组合版）。

- 对标：137 tri → `C(n,k)` 对称约化 + 逐项整除（每步整除精确）
- 绕行：无（纯 `int` 循环；`k > n-k` 先对称约化）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/350_binom/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/350_binom/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/350_binom/main.sla -o /tmp/basic_350 && /tmp/basic_350
# 期望输出：120,10,1
```
