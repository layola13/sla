# 674 eul_fact

Eulerian 行和即阶乘：`esum(4)=24`，`esum(5)=120`，`esum(6)=720`（阶乘桥）。

- 对标：667_eul_row × 230_fact_iter → 两族桥接（行和恒等单锁；675 做三向收束）
- 绕行：`fact(6)=720` 同值陪锁（230 迭代体例复用；差值输出避布尔直打 493 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/674_eul_fact/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/674_eul_fact/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/674_eul_fact/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/674_eul_fact/main.sla -o /tmp/basic_674 && /tmp/basic_674
# 期望输出：24,120,720
```
