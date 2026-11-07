# 378 abundant

丰数计数（`σ(n) > 2n`；`<=20 → 3`；377 的分类版）。

- 对标：377 sigma → 盈亏分类计数（12/18/20；6 完全数判非丰）
- 绕行：无（复用 `sigma`；`1/0` 代替布尔）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/378_abundant/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/378_abundant/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/378_abundant/main.sla -o /tmp/basic_378 && /tmp/basic_378
# 期望输出：3,1,0
```
