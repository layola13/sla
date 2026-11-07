# 923 duel_eulrow9

Eulerian 行和二对峙：建表行和 vs 交错和行和（`n=8：40320` 双锁，`n=9：362880` 双锁）。

- 对标：922_eul_fact9 × 918_duel_eul9 → 行源与交错源合流
- 绕行：无（双实现互锁，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/923_duel_eulrow9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/923_duel_eulrow9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/923_duel_eulrow9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/923_duel_eulrow9/main.sla -o /tmp/basic_923 && /tmp/basic_923
# 期望输出：40320,40320,362880,362880
```
