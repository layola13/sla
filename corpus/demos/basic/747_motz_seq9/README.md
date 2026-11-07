# 747 motz_seq9

Motzkin 序列前 10 项和 `1374`（`M(8)=323`，`M(9)=835`；循环求和）。

- 对标：746_motz9 → 单值版互补（同式序列复算；和 main 直打）
- 绕行：十项和 `539+835=1374` 手算复核；`mozseq(5)=38` 低位守卫（627 复用）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/747_motz_seq9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/747_motz_seq9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/747_motz_seq9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/747_motz_seq9/main.sla -o /tmp/basic_747 && /tmp/basic_747
# 期望输出：323,835,1374
```
