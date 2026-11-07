# 838 duel_lahcol19

Lah 首列对峙：建表 vs 阶乘（`(7,1)=5040`，`(8,1)=40320`，`(9,1)=362880` 三档互锁）。

- 对标：711_lah_col1 → 低位首列互补（首列恒等 `L(n,1)=n!` 纵深三值锁定）
- 绕行：无（纯整数恒等，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/838_duel_lahcol19/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/838_duel_lahcol19/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/838_duel_lahcol19/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/838_duel_lahcol19/main.sla -o /tmp/basic_838 && /tmp/basic_838
# 期望输出：5040,5040,40320,40320,362880
```
