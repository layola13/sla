# 745 tri_fub

有序 Bell 三向：商版递推 × 加权和 × 表版递推（`n=7：47293 三锁`）。

- 对标：736_fub7 → 商源互补（605 三向体例；三算法互锁收束 Fubini 续高位族）
- 绕行：表商同核异源交叉确证（730 体例；`fub(6)=4683` 低位陪锁）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/745_tri_fub/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/745_tri_fub/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/745_tri_fub/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/745_tri_fub/main.sla -o /tmp/basic_745 && /tmp/basic_745
# 期望输出：47293,47293,47293
```
