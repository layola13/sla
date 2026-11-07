# 642 duel_rio7

Riordan 高位二对峙：短递推 vs 卷积（`n=6：15 对 15`；`n=7：36 对 36`）。

- 对标：641_rio7 → 高位源互补（639 对峙体例再上移；双边同值锁定）
- 绕行：卷积 `n=7` 六项 `21+0+4+2+3+6=36` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/642_duel_rio7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/642_duel_rio7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/642_duel_rio7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/642_duel_rio7/main.sla -o /tmp/basic_642 && /tmp/basic_642
# 期望输出：15,15,36,36
```
