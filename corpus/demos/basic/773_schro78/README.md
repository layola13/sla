# 773 schro78

大 Schröder 续高位 `S(7)=8558`，`S(8)=41586`（613 体例续高位；桥恒等大端）。

- 对标：613_schro6 → `S(6)` 版互补（同式再上两位；775 做三向收束）
- 绕行：卷积和 `6752`/`33028` 双档手算复核（自纠 `34028` 笔误，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/773_schro78/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/773_schro78/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/773_schro78/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/773_schro78/main.sla -o /tmp/basic_773 && /tmp/basic_773
# 期望输出：8558,41586
```
