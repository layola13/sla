# 780 duel_bell10

Bell 高位对峙：二项递推 vs Aitken 三角（`n=9：21147 对 21147`；`n=10：115975 对 115975`）。

- 对标：778_bell10 → 高位源互补（779 对峙体例上移；双边同值锁定）
- 绕行：三角 `O(n²)` 建表秒级（`n=10` 百格；递推端 `2^10` 扇出内）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/780_duel_bell10/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/780_duel_bell10/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/780_duel_bell10/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/780_duel_bell10/main.sla -o /tmp/basic_780 && /tmp/basic_780
# 期望输出：21147,21147,115975,115975
```
