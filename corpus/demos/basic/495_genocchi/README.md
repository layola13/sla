# 495 genocchi

Genocchi 数（`G(6)=-3`；491 的组合版）。

- 对标：491 bernoulli → `2(1-2^n)B_n` 组合（`G(2)=-1`，`G(4)=1`）
- 绕行：`gcd` 绝对值复用（491 同例）；分母恒约 1 返分子

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/495_genocchi/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/495_genocchi/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/495_genocchi/main.sla -o /tmp/basic_495 && /tmp/basic_495
# 期望输出：-1,1,-3
```
