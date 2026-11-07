# 700 duel_stir28

Stirling 二类高位对峙：建表 vs 递归（`(8,3)：966 对 966`；`(8,4)：1701 对 1701`）。

- 对标：698_stir28 → 高位源互补（699 对峙体例上移；双边同值锁定）
- 绕行：`rs2(8,4)` 递归扇出探针两后端 8s 级（含全组断言，无超时风险）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/700_duel_stir28/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/700_duel_stir28/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/700_duel_stir28/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/700_duel_stir28/main.sla -o /tmp/basic_700 && /tmp/basic_700
# 期望输出：966,966,1701,1701
```
