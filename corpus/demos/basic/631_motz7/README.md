# 631 motz7

Motzkin 高位二 `M(7)=127`（`M(6)=51` 低位锚；卷积和 `76`）。

- 对标：628_motz_high → 高位版互补（同式再上一位；632 做迭代/直和对峙）
- 绕行：六项卷积 `21+9+8+8+9+21=76` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/631_motz7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/631_motz7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/631_motz7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/631_motz7/main.sla -o /tmp/basic_631 && /tmp/basic_631
# 期望输出：51,127
```
