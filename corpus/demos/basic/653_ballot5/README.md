# 653 ballot5

Ballot 第 5 行 `1,5,14,28,42,42`（行和 `132=Catalan(6)`；循环求和）。

- 对标：647_ballot_row → 低位行互补（同式高位行复算；行和恒等再锁）
- 绕行：行和 `1+5+14+28+42+42=132` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/653_ballot5/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/653_ballot5/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/653_ballot5/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/653_ballot5/main.sla -o /tmp/basic_653 && /tmp/basic_653
# 期望输出：5,14,28
```
