# 764 duel_delan_col7

Delannoy 列对峙：建表 vs 直和（`(4,7)：2241 对 2241`；`(5,7)：7183 对 7183`）。

- 对标：763_delan_col7 → 列源互补（759 对峙体例列移；双边同值锁定）
- 绕行：直和五项 `1+56+504+1120+560=2241` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/764_duel_delan_col7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/764_duel_delan_col7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/764_duel_delan_col7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/764_duel_delan_col7/main.sla -o /tmp/basic_764 && /tmp/basic_764
# 期望输出：2241,2241,7183,7183
```
