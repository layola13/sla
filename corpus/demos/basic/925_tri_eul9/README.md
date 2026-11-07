# 925 tri_eul9

Eulerian 续三向二：建表行和 × 交错和行和 × 阶乘（`n=9：362880` 三锁）。

- 对标：917_eul_row9 → 行源互补（三实现互锁；805 三向体例收束 Eulerian 续三行族）
- 绕行：三实现皆探针三线绿（建表/交错/阶乘秒级会师）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/925_tri_eul9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/925_tri_eul9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/925_tri_eul9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/925_tri_eul9/main.sla -o /tmp/basic_925 && /tmp/basic_925
# 期望输出：362880,362880,362880
```
