# 825 tri_stir19

Stirling 一类续三向：建表行和 × 递归行和 × 阶乘（`n=9：362880 三锁`）。

- 对标：817_stir1_row9 → 行源互补（695 三向体例；三算法互锁收束一类续三行族）
- 绕行：`rsum(9)` 递归行和扇出最大（探针同形 8s 级；`csum(8)=40320` 低位陪锁）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/825_tri_stir19/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/825_tri_stir19/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/825_tri_stir19/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/825_tri_stir19/main.sla -o /tmp/basic_825 && /tmp/basic_825
# 期望输出：362880,362880,362880
```
