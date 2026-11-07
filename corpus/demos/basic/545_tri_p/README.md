# 545 tri_p

`p(10)=42` 三向收束：背包 DP `==` 五边形递推 `==` 最大部行和 `Σ_k P(10,k)`（`42` 对 `42` 对 `42`）。

- 对标：536_p8 × 542_pent_extend × 544_largest → 三源单实现见各源（316 系三向体例）
- 绕行：行和另断言 `row_sum(8)=22`（`1+4+5+5+3+2+1+1` 手算复核）；三表各自独立

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/545_tri_p/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/545_tri_p/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/545_tri_p/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/545_tri_p/main.sla -o /tmp/basic_545 && /tmp/basic_545
# 期望输出：42,42,42
```
