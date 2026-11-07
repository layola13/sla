# 815 tri_eulb67

Eulerian-B 续三向：建表行和 × 交错和行和 × 公式（`n=7：645120 三锁`）。

- 对标：808_eulb7 → 高位源互补（685 三向体例；三算法互锁收束 B 型续族）
- 绕行：三源异构（建表/交错/公式）同值锁定；`ebsum(6)=46080` 低位陪锁

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/815_tri_eulb67/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/815_tri_eulb67/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/815_tri_eulb67/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/815_tri_eulb67/main.sla -o /tmp/basic_815 && /tmp/basic_815
# 期望输出：645120,645120,645120
```
