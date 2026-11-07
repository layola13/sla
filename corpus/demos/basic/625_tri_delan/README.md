# 625 tri_delan

Delannoy 三向：拍扁建表 × 二项直和 × 容斥和（`(5,5)：1683 三锁`）。
容斥 `D(m,n)=sum C(m,k)·C(n+m-k,m)`（`252,630,560,210,30,1` 精确求和）。

- 对标：617_delannoy_high → 高位源互补（605 三向体例；三算法互锁收束 Delannoy 族）
- 绕行：容斥和逐项先乘精确整除（596 口径）；`ddel2(3,3)=63` 小值端点守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/625_tri_delan/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/625_tri_delan/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/625_tri_delan/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/625_tri_delan/main.sla -o /tmp/basic_625 && /tmp/basic_625
# 期望输出：1683,1683,1683
```
