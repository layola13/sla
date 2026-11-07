# 690 duel_stir18

Stirling 一类高位对峙：建表 vs 递归（`(8,2)：13068 对 13068`；`(8,3)：13132 对 13132`）。

- 对标：688_stir18 → 高位源互补（689 对峙体例上移；双边同值锁定）
- 绕行：`rcs(8,3)` 递归扇出探针两后端 8s 级（含全组断言，无超时风险）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/690_duel_stir18/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/690_duel_stir18/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/690_duel_stir18/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/690_duel_stir18/main.sla -o /tmp/basic_690 && /tmp/basic_690
# 期望输出：13068,13068,13132,13132
```
