# 899 schro12

大 Schröder 续高位四 `S(12)=27297738`（`S(11)=5293446` 对照；卷积和 `22004292`）。

- 对标：896_schro11 → `n=11` 版互补（同式再上一位；900 做高位对峙）
- 绕行：记忆值 `27278694` 误（商式/迭代/倍小/递推公式四源会师 `27297738`；记账）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/899_schro12/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/899_schro12/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/899_schro12/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/899_schro12/main.sla -o /tmp/basic_899 && /tmp/basic_899
# 期望输出：27297738,5293446
```
