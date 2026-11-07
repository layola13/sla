# 613 schro6

Schröder 高位二 `S(6)=1806`（`S(5)=394` 低位锚；卷积和 `1412`）。

- 对标：608_schro_high → 高位版互补（同式再上一位；614 做高位二对峙）
- 绕行：六项卷积 `394+180+132+132+180+394=1412` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/613_schro6/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/613_schro6/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/613_schro6/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/613_schro6/main.sla -o /tmp/basic_613 && /tmp/basic_613
# 期望输出：394,1806
```
