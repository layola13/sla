# 791 sschro910

小 Schröder 续高位 `s(9)=103049`，`s(10)=518859`（短递推续高位；771 体例续）。

- 对标：771_sschro8 → `n=8` 版互补（同式再上两位；792 做续桥对峙）
- 绕行：`[51·20793-29953]/10=103049` 先算后除精确（596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/791_sschro910/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/791_sschro910/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/791_sschro910/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/791_sschro910/main.sla -o /tmp/basic_791 && /tmp/basic_791
# 期望输出：103049,518859
```
