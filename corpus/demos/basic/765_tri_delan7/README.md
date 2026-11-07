# 765 tri_delan7

Delannoy 续三向：拍扁建表 × 二项直和 × 容斥和（`(7,7)：48639 三锁`）。
容斥 `D(m,n)=sum C(m,k)·C(n+m-k,m)`（`3432+12012+16632+11550+4200+756+56+1=48639`）。

- 对标：758_delan_high7 → 高位源互补（625 三向体例；三算法互锁收束 Delannoy 续族）
- 绕行：容斥八项手算复核（会师仲裁；`ddel2(5,5)=1683` 低位陪锁）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/765_tri_delan7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/765_tri_delan7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/765_tri_delan7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/765_tri_delan7/main.sla -o /tmp/basic_765 && /tmp/basic_765
# 期望输出：48639,48639,48639
```
