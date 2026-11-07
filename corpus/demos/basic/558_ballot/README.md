# 558 ballot

Ballot 数 `ballot(p,q)=(p-q+1)·C(p+q,q)/(p+1)`（`p≥q`，先乘后除精确整除）。
`ballot(3,3)=5`，`ballot(4,2)=9`，`ballot(4,4)=14`（末项即 `C(4)`）。

- 对标：351_catalan → 商式版互补（此处为双参 Ballot 族；`q=p` 退化即 Catalan）
- 绕行：`ballot(5,0)=1` 单候选端点；中间积值域内无溢出（记 README）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/558_ballot/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/558_ballot/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/558_ballot/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/558_ballot/main.sla -o /tmp/basic_558 && /tmp/basic_558
# 期望输出：5,9,14
```
