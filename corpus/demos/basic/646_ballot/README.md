# 646 ballot

Ballot 三角建表 `B(n,k)=B(n-1,k)+B(n,k-1)`（上三角守卫，546 范式变体）。
`B(3,2)=5`，`B(4,2)=9`，`B(4,3)=14`。

- 对标：351_catalan → 对角线即 Catalan（此处为全三角；649 做建表/商式对峙）
- 绕行：`nn<=mm` 上三角守卫（`k>n` 格恒零，探针三线绿）；首列留 `1`（546 同式）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/646_ballot/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/646_ballot/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/646_ballot/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/646_ballot/main.sla -o /tmp/basic_646 && /tmp/basic_646
# 期望输出：5,9,14
```
