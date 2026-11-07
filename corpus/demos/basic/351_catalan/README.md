# 351 catalan

Catalan 数（`C(2n,n)/(n+1)`；350 的商式版）。

- 对标：350 binom → 整除商式（每步精确整除后再除 `n+1`）
- 绕行：无（纯 `int` 循环复用 `binom`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/351_catalan/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/351_catalan/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/351_catalan/main.sla -o /tmp/basic_351 && /tmp/basic_351
# 期望输出：5,14,42
```
