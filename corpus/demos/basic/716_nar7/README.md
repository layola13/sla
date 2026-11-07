# 716 nar7

Narayana 续高位商式 `N(n,k)=C(n,k)·C(n,k+1)/n`（先乘后除精确整除）。
`N(7,1)=21`，`N(7,2)=105`，`N(7,3)=175`。

- 对标：596_narayana → `n≤6` 版互补（此处为 `n=7` 高位行；719 做行和对峙）
- 绕行：`35·35/7=175` 先乘后除精确（596 口径）；`N(6,2)=50` 低位锚守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/716_nar7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/716_nar7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/716_nar7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/716_nar7/main.sla -o /tmp/basic_716 && /tmp/basic_716
# 期望输出：21,105,175
```
