# 632 duel_motz7

Motzkin 高位二对峙：迭代建表 vs 二项直和（`n=6：51 对 51`；`n=7：127 对 127`）。
直和 `M(n)=sum C(n,2k)·Catalan(k)`（`n=7：1,21,70,35` 精确求和）。

- 对标：631_motz7 → 高位源互补（633 直和源合流；递归止 `n=6`，本轮起高位用直和避扇出）
- 绕行：直和逐项先乘后除（596 口径）；`2*k` 偶下标守卫防越界求和

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/632_duel_motz7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/632_duel_motz7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/632_duel_motz7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/632_duel_motz7/main.sla -o /tmp/basic_632 && /tmp/basic_632
# 期望输出：51,51,127,127
```
