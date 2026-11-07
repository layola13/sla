# 474 duel star

星形数双实现对峙（闭式 vs 三角累加；473 的星形版）。

- 对标：473 duel_pent → `1+12·T(n-1)` 锁定（`4 → 73=73`）
- 绕行：`n <= 1` 守卫（`tri(0)=0` 得 1 正确，守卫保语义显式）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/474_duel_star/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/474_duel_star/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/474_duel_star/main.sla -o /tmp/basic_474 && /tmp/basic_474
# 期望输出：73,73
```
