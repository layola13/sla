# 607 schro_seq

Schröder 序列前 5 项 `1,2,6,22,90`（前缀和 `schseq(4)=121`；循环求和）。

- 对标：606_schroder → 单值版互补（同式序列复算；和 main 直打）
- 绕行：`sch(0)` 空表端点（`t=[1]` 直接返回）；`schseq(2)=9` 小值守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/607_schro_seq/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/607_schro_seq/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/607_schro_seq/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/607_schro_seq/main.sla -o /tmp/basic_607 && /tmp/basic_607
# 期望输出：1,2,121
```
