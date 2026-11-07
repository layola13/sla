# 753 duel_rio910

Riordan 续桥对峙：短递推 vs 桥序（`n=9：232 对 232`；`n=10：603 对 603`）。

- 对标：751_motz_riob9 → 续桥源互补（639 对峙体例上移；双边同值锁定）
- 绕行：桥序线性递归深度止 `10`（单链回溯，两后端秒级）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/753_duel_rio910/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/753_duel_rio910/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/753_duel_rio910/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/753_duel_rio910/main.sla -o /tmp/basic_753 && /tmp/basic_753
# 期望输出：232,232,603,603
```
