# 792 duel_sschro910

大小 Schröder 续桥对峙：短递推 vs 半大数（`n=9：103049 对 103049`；`n=10：518859 对 518859`）。

- 对标：791_sschro910 → 续桥源互补（786 续高位源合流；双边同值锁定）
- 绕行：`S(10)/2=1037718/2=518859` 大数半数精确（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/792_duel_sschro910/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/792_duel_sschro910/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/792_duel_sschro910/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/792_duel_sschro910/main.sla -o /tmp/basic_792 && /tmp/basic_792
# 期望输出：103049,103049,518859,518859
```
