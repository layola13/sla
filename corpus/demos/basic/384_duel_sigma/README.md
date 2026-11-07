# 384 duel sigma

除数和双实现对峙（全量 vs 半程；374 的除数和版）。

- 对标：374 duel_primecount → `σ(12) = 28` 双边锁定（半程版加回 `n`）
- 绕行：无（双实现皆纯 `int` 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/384_duel_sigma/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/384_duel_sigma/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/384_duel_sigma/main.sla -o /tmp/basic_384 && /tmp/basic_384
# 期望输出：28,28
```
