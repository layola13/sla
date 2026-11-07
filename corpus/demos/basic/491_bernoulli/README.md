# 491 bernoulli

Bernoulli 数（`B(2)=1/6`，`B(4)=-1/30`；480 的递推版）。

- 对标：480 harm_frac → `ΣC(m+1,k)B_k=0` 分式递推（`B(6)=1/42`）
- 绕行：`gcd` 先取绝对值（负分子致负 gcd 翻符号坑，写码即改）；
  分子分母双函数（477 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/491_bernoulli/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/491_bernoulli/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/491_bernoulli/main.sla -o /tmp/basic_491 && /tmp/basic_491
# 期望输出：1,6,-1,30
```
