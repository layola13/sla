# 654 ballot_diag

Ballot 对角线即 Catalan：`B(3,3)=5`，`B(4,4)=14`，`B(5,5)=42`（对角恒等单锁）。

- 对标：351_catalan → Catalan 单值版互补（对角线复算；655 做三向收束）
- 绕行：对角 `B(n,n)=B(n,n-1)` 自然成立（守卫保证 `k>n` 格零污染）；`B(2,2)=2` 小值守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/654_ballot_diag/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/654_ballot_diag/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/654_ballot_diag/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/654_ballot_diag/main.sla -o /tmp/basic_654 && /tmp/basic_654
# 期望输出：5,14,42
```
