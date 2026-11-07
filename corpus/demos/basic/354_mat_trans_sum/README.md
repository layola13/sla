# 354 mat trans sum

转置和恒等式（`sum(A)+sum(A^T) = 2*sum(A)`；19 的恒等式版）。

- 对标：19 transpose → 转置换元求和对照
- 绕行：无（标量四元组；避 #1 数组门）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/354_mat_trans_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/354_mat_trans_sum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/354_mat_trans_sum/main.sla -o /tmp/basic_354 && /tmp/basic_354
# 期望输出：20,10
```
