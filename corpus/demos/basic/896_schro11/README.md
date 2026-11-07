# 896 schro11

大 Schröder 续高位三 `S(11)=5293446`（`S(10)=1037718` 对照；卷积和 `4255728`）。

- 对标：788_schro10 → `n=10` 版互补（同式再上一位；898 做迭代/商式对峙）
- 绕行：`5293446-1037718=4255728` 卷积和手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/896_schro11/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/896_schro11/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/896_schro11/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/896_schro11/main.sla -o /tmp/basic_896 && /tmp/basic_896
# 期望输出：5293446,1037718
```
