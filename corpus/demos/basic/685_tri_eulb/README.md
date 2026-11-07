# 685 tri_eulb

Eulerian-B 三向：建表行和 × 交错和行和 × 公式（`n=5：3840 三锁`）。

- 对标：678_eulb_high → 高位源互补（605 三向体例；三算法互锁收束 B 型族）
- 绕行：`eba(5,5)=1` 越界端点自然归一（上确界恒一，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/685_tri_eulb/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/685_tri_eulb/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/685_tri_eulb/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/685_tri_eulb/main.sla -o /tmp/basic_685 && /tmp/basic_685
# 期望输出：3840,3840,3840
```
