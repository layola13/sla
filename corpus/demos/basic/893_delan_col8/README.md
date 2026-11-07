# 893 delan_col8

Delannoy 第 8 列纵深 `D(5,8)=13073`，`D(6,8)=40081`，`D(7,8)=108545`（列视角复算）。

- 对标：886_delan8 → 行视角互补（同值列复算；894 做列对峙）
- 绕行：对称恒等 `D(7,8)=D(8,7)` 双算一致（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/893_delan_col8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/893_delan_col8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/893_delan_col8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/893_delan_col8/main.sla -o /tmp/basic_893 && /tmp/basic_893
# 期望输出：13073,40081,108545
```
