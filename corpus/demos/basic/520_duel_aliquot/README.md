# 520 duel aliquot

真因子和双实现对峙（全量 vs 半程；384 的真因子版）。

- 对标：384 duel_sigma → 真因子天然半程（`s(220)=284` 双边）
- 绕行：无（`i < n` 与 `i <= n/2` 等价；纯 `int` 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/520_duel_aliquot/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/520_duel_aliquot/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/520_duel_aliquot/main.sla -o /tmp/basic_520 && /tmp/basic_520
# 期望输出：284,284
```
