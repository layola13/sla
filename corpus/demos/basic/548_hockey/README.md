# 548 hockey

hockey-stick 恒等式 `Σ_{i=k}^{n} C(i,k) = C(n+1,k+1)`（`(1,5)→15`；`(2,6)→35`；`(3,7)→70`）。

- 对标：547_rowsum → 横向求和版互补（此处为对角求和；551 做查表对峙）
- 绕行：Pascal 首列恒一式（546 同例）；`hockey` 下界 `i=k` 含端点（`C(k,k)=1`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/548_hockey/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/548_hockey/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/548_hockey/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/548_hockey/main.sla -o /tmp/basic_548 && /tmp/basic_548
# 期望输出：15,35,70
```
