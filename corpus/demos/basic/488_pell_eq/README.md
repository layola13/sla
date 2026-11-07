# 488 pell eq

Pell 方程基本解（`D=5 → 904`；329 的丢番图版）。

- 对标：329 quad_roots → `x²-1=D·y²`  ascending 搜索（百位打包 `x*100+y`）
- 绕行：`is_square` 返根（0 表非方）；上界 1000 内必中选例

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/488_pell_eq/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/488_pell_eq/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/488_pell_eq/main.sla -o /tmp/basic_488 && /tmp/basic_488
# 期望输出：302,201,904
```
