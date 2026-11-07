# 604 narayana_alt

Narayana 商式B `N(n,k)=C(n-1,k)·C(n+1,k+1)/(n+1)`（596 商式A的移位变体）。
`N(5,2)=20`，`N(6,2)=50`，`N(6,3)=50`。

- 对标：596_narayana → 商式A版互补（除数 `n+1` 移位；605 做三向收束）
- 绕行：`narayana_b(4,1)=6` 移位端点守卫（`3·10/5` 手算复核）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/604_narayana_alt/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/604_narayana_alt/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/604_narayana_alt/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/604_narayana_alt/main.sla -o /tmp/basic_604 && /tmp/basic_604
# 期望输出：20,50,50
```
