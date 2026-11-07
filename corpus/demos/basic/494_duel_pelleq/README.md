# 494 duel pelleq

Pell 基本解双实现对峙（x 序 vs y 序；403 的搜索版）。

- 对标：403 duel_fermat → 双搜索序锁定（`D=5 → 904=904`）
- 绕行：`is_square` 返根（0 表非方）；百位打包

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/494_duel_pelleq/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/494_duel_pelleq/main.sla -o /tmp/basic_494 && /tmp/basic_494
# 期望输出：904,904
```
