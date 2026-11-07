# 596 narayana

Narayana 数商式 `N(n,k)=C(n,k)·C(n,k+1)/n`（先乘后除精确整除）。
`N(4,1)=6`，`N(4,2)=6`，`N(5,2)=20`。

- 对标：586_lah → 同为商式族（此处为双二项商式；Catalan 细化，599 做行和对峙）
- 绕行：`N(3,1)=3` 小值端点守卫；中间积值域内无溢出（`10·10=100`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/596_narayana/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/596_narayana/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/596_narayana/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/596_narayana/main.sla -o /tmp/basic_596 && /tmp/basic_596
# 期望输出：6,6,20
```
