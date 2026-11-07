# 610 duel_schro_high

Schröder 高位对峙：迭代 vs 递归（`n=4：90 对 90`；`n=5：394 对 394`）。

- 对标：608_schro_high → 高位源互补（609 对峙体例上移；双边同值锁定）
- 绕行：`rsch(5)` 递归扇出大但两后端秒级（探针：sab 0.3s / sa 8s，无超时风险）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/610_duel_schro_high/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/610_duel_schro_high/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/610_duel_schro_high/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/610_duel_schro_high/main.sla -o /tmp/basic_610 && /tmp/basic_610
# 期望输出：90,90,394,394
```
