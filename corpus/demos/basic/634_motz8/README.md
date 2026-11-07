# 634 motz8

Motzkin 高位三 `M(8)=323`（`M(7)=127` 低位锚；卷积和 `196`）。

- 对标：631_motz7 → 高位版互补（同式再上一位；635 做三向收束）
- 绕行：七项卷积 `51+21+18+16+18+21+51=196` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/634_motz8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/634_motz8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/634_motz8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/634_motz8/main.sla -o /tmp/basic_634 && /tmp/basic_634
# 期望输出：127,323
```
