# 895 tri_delan8

Delannoy 续三向二：拍扁建表 × 二项直和 × 容斥和（`(8,8)：265729` 三锁）。

- 对标：888_delan_high8 → 高位源互补（三实现互锁；765 三向体例收束 Delannoy 续三行族）
- 绕行：容斥八项和手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/895_tri_delan8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/895_tri_delan8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/895_tri_delan8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/895_tri_delan8/main.sla -o /tmp/basic_895 && /tmp/basic_895
# 期望输出：265729,265729,265729
```
