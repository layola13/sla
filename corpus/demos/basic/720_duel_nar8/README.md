# 720 duel_nar8

Narayana 高位对峙：行和 vs Catalan 商式（`n=7：429 对 429`；`n=8：1430 对 1430`）。

- 对标：718_nar8 → 高位源互补（719 对峙体例上移；双边同值锁定）
- 绕行：`C(16,8)/9=1430` 大值精确整除（中间积 `12870` 值域内，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/720_duel_nar8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/720_duel_nar8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/720_duel_nar8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/720_duel_nar8/main.sla -o /tmp/basic_720 && /tmp/basic_720
# 期望输出：429,429,1430,1430
```
