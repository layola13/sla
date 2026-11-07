# 805 tri_eul78

Eulerian 续三向：建表行和 × 交错和行和 × 阶乘（`n=8：40320 三锁`）。

- 对标：798_eul8 → 高位源互补（675 三向体例；三算法互锁收束 Eulerian 续族）
- 绕行：`ealt(8,8)=1` 越界端点自然归一（上确界恒一，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/805_tri_eul78/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/805_tri_eul78/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/805_tri_eul78/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/805_tri_eul78/main.sla -o /tmp/basic_805 && /tmp/basic_805
# 期望输出：40320,40320,40320
```
