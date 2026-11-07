# 601 narayana_sym

Narayana 对称性 `N(n,k)==N(n,n-1-k)`（`N(5,1)=10` 对 `N(5,3)=10`；`N(6,1)=15` 对 `N(6,4)=15`）。

- 对标：596_narayana → 单实现见源（553/574 对称体例；镜像下标 `n-1-k`）
- 绕行：镜像下标显式传参（574 同例教训，记 README 防差一）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/601_narayana_sym/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/601_narayana_sym/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/601_narayana_sym/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/601_narayana_sym/main.sla -o /tmp/basic_601 && /tmp/basic_601
# 期望输出：10,10,15,15
```
