# 614 duel_schro6

Schröder 高位二对峙：迭代 vs 递归（`n=5：394 对 394`；`n=6：1806 对 1806`）。

- 对标：613_schro6 → 高位源互补（609 对峙体例再上移；双边同值锁定）
- 绕行：`rsch(6)` 递归扇出最大但两后端秒级（探针：sab 0.3s / sa 8s，无超时风险）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/614_duel_schro6/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/614_duel_schro6/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/614_duel_schro6/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/614_duel_schro6/main.sla -o /tmp/basic_614 && /tmp/basic_614
# 期望输出：394,394,1806,1806
```
