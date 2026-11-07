# 382 phi sum

totient 累和（`S(10)=32`；336 的累和版）。

- 对标：336 euler_phi → 逐项 `phi` 累加（`phi(1)=1` 约定，`k < n` 循环漏 `k=n` 但 `n>1` 时 `gcd(n,n)≠1` 无影响）
- 绕行：`n <= 1` 提前返回 1（纯 `int` 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/382_phi_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/382_phi_sum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/382_phi_sum/main.sla -o /tmp/basic_382 && /tmp/basic_382
# 期望输出：32,10
```
