# 763 delan_col7

Delannoy 第 7 列纵深 `D(4,7)=2241`，`D(5,7)=7183`，`D(6,7)=19825`（列视角复算）。

- 对标：756_delan7 → 行视角互补（同值列复算；纵深三值锁定）
- 绕行：`D(3,7)=575` 小值守卫；列值即转置行值（对称恒等复用）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/763_delan_col7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/763_delan_col7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/763_delan_col7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/763_delan_col7/main.sla -o /tmp/basic_763 && /tmp/basic_763
# 期望输出：2241,7183,19825
```
