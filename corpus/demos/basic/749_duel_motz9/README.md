# 749 duel_motz9

Motzkin 续高位对峙：迭代 vs 直和（`n=8：323 对 323`；`n=9：835 对 835`）。

- 对标：746_motz9 → 高位源互补（633 直和源合流；双边同值锁定）
- 绕行：直和五项 `1+36+252+420+126=835` 精确求和（596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/749_duel_motz9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/749_duel_motz9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/749_duel_motz9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/749_duel_motz9/main.sla -o /tmp/basic_749 && /tmp/basic_749
# 期望输出：323,323,835,835
```
