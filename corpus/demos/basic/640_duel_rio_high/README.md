# 640 duel_rio_high

Riordan 高位对峙：短递推 vs 卷积（`n=5：6 对 6`；`n=6：15 对 15`）。

- 对标：638_rio_high → 高位源互补（639 对峙体例上移；双边同值锁定）
- 绕行：卷积 `n=6` 五项 `9+0+2+1+3=15` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/640_duel_rio_high/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/640_duel_rio_high/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/640_duel_rio_high/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/640_duel_rio_high/main.sla -o /tmp/basic_640 && /tmp/basic_640
# 期望输出：6,6,15,15
```
