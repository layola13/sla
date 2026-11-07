# 786 schro9

大 Schröder 续高位 `S(9)=206098`（`S(8)=41586` 低位锚；卷积和 `164512`）。

- 对标：773_schro78 → `S(7/8)` 版互补（同式再上一位；789 做迭代/商式对峙）
- 绕行：九项卷积 `83172+34232+21672+17336+8100=164512` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/786_schro9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/786_schro9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/786_schro9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/786_schro9/main.sla -o /tmp/basic_786 && /tmp/basic_786
# 期望输出：41586,206098
```
