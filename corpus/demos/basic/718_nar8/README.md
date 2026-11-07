# 718 nar8

Narayana 第 8 行高位 `N(8,1)=28`，`N(8,2)=196`，`N(8,3)=490`（同式再上一位）。

- 对标：716_nar7 → `n=7` 版互补（同式再上一位；720 做高位对峙）
- 绕行：`56·70/8=490` 先乘后除精确（596 口径）；`N(8,4)=490` 对称峰守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/718_nar8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/718_nar8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/718_nar8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/718_nar8/main.sla -o /tmp/basic_718 && /tmp/basic_718
# 期望输出：28,196,490
```
