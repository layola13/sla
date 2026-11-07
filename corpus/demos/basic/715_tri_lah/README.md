# 715 tri_lah

Lah 三向：建表行和 × 商式行和 × 递归行和（`n=7：37633 三锁`）。

- 对标：707_lah_row7 → 行源互补（605 三向体例；三算法互锁收束 Lah 续高位族）
- 绕行：`lrsum(7)` 递归行和扇出最大（探针同形 9s 级；`lsum(6)=4051` 低位陪锁）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/715_tri_lah/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/715_tri_lah/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/715_tri_lah/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/715_tri_lah/main.sla -o /tmp/basic_715 && /tmp/basic_715
# 期望输出：37633,37633,37633
```
