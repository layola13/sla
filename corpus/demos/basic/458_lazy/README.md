# 458 lazy

懒人厨师数（`5 刀 → 16`；330 的二次版）。

- 对标：330 arith_seq → 二次闭式（`10 → 56`，`0 → 1`）
- 绕行：无（纯 `int` 闭式；分子恒偶可整除）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/458_lazy/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/458_lazy/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/458_lazy/main.sla -o /tmp/basic_458 && /tmp/basic_458
# 期望输出：16,56,1
```
