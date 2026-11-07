# 775 tri_sschro

大小 Schröder 三向：短递推 × 半大数 × 半递归（`n=6：903 三锁`）。

- 对标：768_sschro_high → 高位源互补（609 三向体例；三算法互锁收束大小族）
- 绕行：半递归 `rsch(6)/2` 探针同形 8s 级（探针三线绿；`ssho(5)=197` 低位陪锁）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/775_tri_sschro/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/775_tri_sschro/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/775_tri_sschro/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/775_tri_sschro/main.sla -o /tmp/basic_775 && /tmp/basic_775
# 期望输出：903,903,903
```
