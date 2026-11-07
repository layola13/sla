# 566 derange

错排数 `!n=(n-1)·(!(n-1)+!(n-2))`（`!3..!6=2,9,44,265`）。

- 对标：125_fact → 阶乘版互补（此处为固定点自由计数；568 做容斥对峙）
- 绕行：`!0=1`/`!1=0` 双基准守卫（525 同例 `if/return` 语句式）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/566_derange/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/566_derange/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/566_derange/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/566_derange/main.sla -o /tmp/basic_566 && /tmp/basic_566
# 期望输出：2,9,44,265
```
