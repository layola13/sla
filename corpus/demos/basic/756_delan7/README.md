# 756 delan7

Delannoy 续高位 `D(7,2)=113`，`D(7,3)=575`，`D(7,4)=2241`（同式再上两行）。

- 对标：611_delannoy_tab → `n≤6` 版互补（同式再上两行；759 做建表/直和对峙）
- 绕行：`D(7,2)=D(2,7)=113` 对称复核（对称即校验，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/756_delan7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/756_delan7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/756_delan7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/756_delan7/main.sla -o /tmp/basic_756 && /tmp/basic_756
# 期望输出：113,575,2241
```
