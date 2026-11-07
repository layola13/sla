# 675 tri_eul

Eulerian 三向：建表行和 × 交错和行和 × 阶乘（`n=5：120 三锁`）。

- 对标：668_eul_high → 高位源互补（605 三向体例；三算法互锁收束 Eulerian 族）
- 绕行：`ealt(5,5)=0` 越界项自然归零（上确界外恒零，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/675_tri_eul/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/675_tri_eul/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/675_tri_eul/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/675_tri_eul/main.sla -o /tmp/basic_675 && /tmp/basic_675
# 期望输出：120,120,120
```
