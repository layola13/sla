# 897 schro_seq11

大 Schröder 序列前 12 项和 `6589727`（`S(11)=5293446`；前缀和循环求和）。

- 对标：896_schro11 → 单值版互补（同式序列复算；前缀和三档连验）
- 绕行：前缀和手算误加一则（`6889727` 误 vs 程序 `6589727`，分步复算确证；记账）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/897_schro_seq11/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/897_schro_seq11/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/897_schro_seq11/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/897_schro_seq11/main.sla -o /tmp/basic_897 && /tmp/basic_897
# 期望输出：6589727,1296281
```
