# 695 tri_stir1

Stirling 一类三向：建表行和 × 递归行和 × 阶乘（`n=7：5040 三锁`）。

- 对标：687_stir1_row7 → 行源互补（605 三向体例；三算法互锁收束一类续高位族）
- 绕行：`rsum(7)` 递归行和扇出最大（探针同形 8s 级；`csum(6)=720` 低位陪锁）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/695_tri_stir1/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/695_tri_stir1/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/695_tri_stir1/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/695_tri_stir1/main.sla -o /tmp/basic_695 && /tmp/basic_695
# 期望输出：5040,5040,5040
```
