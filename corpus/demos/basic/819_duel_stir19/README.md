# 819 duel_stir19

Stirling 一类续高位对峙：建表 vs 递归（`(9,2)：109584 对 109584`；`(9,3)：118124 对 118124`）。

- 对标：816_stir19 → 建表源（递归版互补；689 体例续高位，双边同值锁定）
- 绕行：递归扇出探针两后端 8s 级（含全行断言，无超时风险）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/819_duel_stir19/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/819_duel_stir19/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/819_duel_stir19/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/819_duel_stir19/main.sla -o /tmp/basic_819 && /tmp/basic_819
# 期望输出：109584,109584,118124,118124
```
