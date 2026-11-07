# 922 eul_fact9

Eulerian 行和桥续高位：`esum(9)=362880`，`fact(9)=362880`（804 体例续高位）。

- 对标：917_eul_row9 × 230_fact_iter → 两族桥接续高位（925 做三向收束）
- 绕行：无（行和与阶乘互锁，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/922_eul_fact9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/922_eul_fact9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/922_eul_fact9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/922_eul_fact9/main.sla -o /tmp/basic_922 && /tmp/basic_922
# 期望输出：362880,362880,40320
```
