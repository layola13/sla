# 769 duel_sschro

大小 Schröder 对峙：短递推 vs 半大数（`n=4：45 对 45`；`n=5：197 对 197`）。
半大数 `shalf(n)=S(n)/2`（`S(5)/2=394/2=197` 精确整除）。

- 对标：766_sschro → 短递推源（606 大数源合流；286 系对峙体例，双边同值锁定）
- 绕行：大数恒偶（`S(n)=2s(n)` 探针六值连验）；半数精确整除（596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/769_duel_sschro/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/769_duel_sschro/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/769_duel_sschro/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/769_duel_sschro/main.sla -o /tmp/basic_769 && /tmp/basic_769
# 期望输出：45,45,197,197
```
