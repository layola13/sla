# 888 delan_high8

Delannoy 续中央 `D(8,8)=265729`（`D(8,7)=108545` 对照；三法会师值）。

- 对标：758_delan_high7 → `D(7,7)` 版互补（中央序列再上一位；889 做建表/直和对峙）
- 绕行：对称恒等 `D(8,7)=D(7,8)` 双算一致（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/888_delan_high8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/888_delan_high8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/888_delan_high8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/888_delan_high8/main.sla -o /tmp/basic_888 && /tmp/basic_888
# 期望输出：265729,108545
```
