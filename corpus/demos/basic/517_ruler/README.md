# 517 ruler

尺子函数（`v₂(8)=3`；392 的单值版）。

- 对标：392 fact_prime_exp → 因子 2 剥离计数（`ruler(12)=2`）
- 绕行：`n <= 0` 守卫（`0` 无限剥离坑，407 类）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/517_ruler/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/517_ruler/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/517_ruler/main.sla -o /tmp/basic_517 && /tmp/basic_517
# 期望输出：3,2,0
```
