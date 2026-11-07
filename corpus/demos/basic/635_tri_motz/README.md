# 635 tri_motz

Motzkin 三向：迭代建表 × 递归 × 二项直和（`n=5：21 三锁`）。

- 对标：628_motz_high → 高位源互补（605 三向体例；三算法互锁收束 Motzkin 族）
- 绕行：直和三项 `1+10+10=21` 精确求和（596 口径）；`cmot(4)=9` 小值端点守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/635_tri_motz/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/635_tri_motz/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/635_tri_motz/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/635_tri_motz/main.sla -o /tmp/basic_635 && /tmp/basic_635
# 期望输出：21,21,21
```
