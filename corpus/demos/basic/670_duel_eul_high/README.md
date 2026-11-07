# 670 duel_eul_high

Eulerian 高位对峙：建表 vs 交错和（`(5,2)：66 对 66`；`(6,2)：302 对 302`）。

- 对标：668_eul_high → 高位源互补（669 对峙体例上移；双边同值锁定）
- 绕行：`(6,2)` 交错四项 `729-448+21-0=302` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/670_duel_eul_high/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/670_duel_eul_high/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/670_duel_eul_high/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/670_duel_eul_high/main.sla -o /tmp/basic_670 && /tmp/basic_670
# 期望输出：66,66,302,302
```
