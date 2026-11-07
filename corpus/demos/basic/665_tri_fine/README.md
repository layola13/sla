# 665 tri_fine

Fine 三向：短递推 × 卷积 × 桥序（`n=5：18 三锁`）。

- 对标：658_fine_high → 高位源互补（605 三向体例；三算法互锁收束 Fine 族）
- 绕行：卷积 `4+0+1+1=6` 小值端点（`fconv(4)` 隐含守卫）；桥序差偶数恒成立

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/665_tri_fine/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/665_tri_fine/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/665_tri_fine/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/665_tri_fine/main.sla -o /tmp/basic_665 && /tmp/basic_665
# 期望输出：18,18,18
```
