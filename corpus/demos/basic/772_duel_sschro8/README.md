# 772 duel_sschro8

大小 Schröder 高位二对峙：短递推 vs 半大数（`n=7：4279 对 4279`；`n=8：20793 对 20793`）。

- 对标：771_sschro8 → 高位源互补（769 对峙体例再上移；双边同值锁定）
- 绕行：`S(8)/2=41586/2=20793` 大数半数精确（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/772_duel_sschro8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/772_duel_sschro8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/772_duel_sschro8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/772_duel_sschro8/main.sla -o /tmp/basic_772 && /tmp/basic_772
# 期望输出：4279,4279,20793,20793
```
