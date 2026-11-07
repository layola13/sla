# 770 duel_sschro_high

大小 Schröder 高位对峙：短递推 vs 半大数（`n=6：903 对 903`；`n=7：4279 对 4279`）。

- 对标：768_sschro_high → 高位源互补（769 对峙体例上移；双边同值锁定）
- 绕行：`S(7)/2=8558/2=4279` 大数半数精确（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/770_duel_sschro_high/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/770_duel_sschro_high/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/770_duel_sschro_high/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/770_duel_sschro_high/main.sla -o /tmp/basic_770 && /tmp/basic_770
# 期望输出：903,903,4279,4279
```
