# 463 duel cake

蛋糕数双实现对峙（闭式 vs 逐层累加；460 的三次版）。

- 对标：460 duel_lazy → `C(n)=C(n-1)+L(n-1)` 锁定（`5 → 26=26`）
- 绕行：递归基 `C(0)=1`（411 同例）；复用 `lazy` 闭式

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/463_duel_cake/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/463_duel_cake/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/463_duel_cake/main.sla -o /tmp/basic_463 && /tmp/basic_463
# 期望输出：26,26
```
