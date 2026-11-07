# 762 delan_central7

Delannoy 续中央列 `1683,8989,48639`（`D(5,5)`，`D(6,6)`，`D(7,7)`；中央序列复算）。

- 对标：620_delannoy_central → 低位中央版互补（中央序列复算；765 做三向收束）
- 绕行：`D(4,4)=321` 小值守卫；中央值三法会师（表/直和/容斥）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/762_delan_central7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/762_delan_central7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/762_delan_central7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/762_delan_central7/main.sla -o /tmp/basic_762 && /tmp/basic_762
# 期望输出：1683,8989,48639
```
