# 645 tri_rio

Riordan 三向：短递推 × 卷积 × 桥序（`n=5：6 三锁`）。

- 对标：638_rio_high → 高位源互补（605 三向体例；三算法互锁收束 Riordan 族）
- 绕行：桥序 `M(4)-R(4)=9-3=6` 差式端点守卫（436 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/645_tri_rio/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/645_tri_rio/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/645_tri_rio/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/645_tri_rio/main.sla -o /tmp/basic_645 && /tmp/basic_645
# 期望输出：6,6,6
```
