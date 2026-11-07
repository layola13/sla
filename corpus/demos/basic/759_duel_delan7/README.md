# 759 duel_delan7

Delannoy 续高位对峙：建表 vs 直和（`(6,6)：8989 对 8989`；`(7,7)：48639 对 48639`）。

- 对标：758_delan_high7 → 高位源互补（612 对峙体例上移；双边同值锁定）
- 绕行：直和八项 `1+98+1764+9800+19600+14112+3136+128=48639` 手算复核（会师仲裁）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/759_duel_delan7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/759_duel_delan7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/759_duel_delan7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/759_duel_delan7/main.sla -o /tmp/basic_759 && /tmp/basic_759
# 期望输出：8989,8989,48639,48639
```
