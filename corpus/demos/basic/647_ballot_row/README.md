# 647 ballot_row

Ballot 第 4 行 `1,4,9,14,14`（行和 `42=Catalan(5)`；循环求和）。

- 对标：646_ballot → 单格版互补（同式行复算；行和 main 直打）
- 绕行：行和恒等 `sum(B(4,k))=Catalan(5)` 双锁（小值端点 `B(4,0)=1` 守卫）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/647_ballot_row/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/647_ballot_row/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/647_ballot_row/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/647_ballot_row/main.sla -o /tmp/basic_647 && /tmp/basic_647
# 期望输出：4,9,14
```
