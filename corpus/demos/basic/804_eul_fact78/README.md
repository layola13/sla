# 804 eul_fact78

Eulerian 行和桥续高位：`esum(7)=5040`，`esum(8)=40320`（`fact(7/8)` 陪锁）。

- 对标：797_eul_row7 × 230_fact_iter → 两族桥接续高位（行和恒等单锁；805 做三向收束）
- 绕行：行和与阶乘跨族同值（674 体例续高位；差值输出避布尔直打 493 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/804_eul_fact78/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/804_eul_fact78/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/804_eul_fact78/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/804_eul_fact78/main.sla -o /tmp/basic_804 && /tmp/basic_804
# 期望输出：5040,5040,40320,40320
```
