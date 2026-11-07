# 620 delannoy_central

Delannoy 中央列 `1,3,13,63,321,1683`（`D(3,3)=63`，`D(4,4)=321`，`D(5,5)=1683`）。

- 对标：617_delannoy_high → 高位版互补（中央序列复算；621 做中央对峙）
- 绕行：`D(1,1)=3` 小值端点守卫；中央列即对角线单锁

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/620_delannoy_central/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/620_delannoy_central/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/620_delannoy_central/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/620_delannoy_central/main.sla -o /tmp/basic_620 && /tmp/basic_620
# 期望输出：63,321,1683
```
