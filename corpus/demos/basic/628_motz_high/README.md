# 628 motz_high

Motzkin 高位 `M(6)=51`（`M(5)=21` 低位锚；卷积和 `30`）。

- 对标：626_motzkin → 低位版互补（同式高位复算；630 做高位对峙）
- 绕行：五项卷积 `9+4+4+4+9=30` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/628_motz_high/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/628_motz_high/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/628_motz_high/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/628_motz_high/main.sla -o /tmp/basic_628 && /tmp/basic_628
# 期望输出：21,51
```
