# 886 delan8

Delannoy 续高位三 `D(8,2)=145`，`D(8,3)=833`，`D(8,4)=3649`（同式再上一行）。

- 对标：756_delan7 → `n=7` 版互补（同式再上一行；889 做建表/直和对峙）
- 绕行：`1+48+336+448=833` 直和手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/886_delan8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/886_delan8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/886_delan8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/886_delan8/main.sla -o /tmp/basic_886 && /tmp/basic_886
# 期望输出：145,833,3649
```
