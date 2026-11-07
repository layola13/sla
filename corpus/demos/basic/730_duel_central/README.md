# 730 duel_central

中央二项对峙：商式直算 vs Pascal 建表（`(10,5)：252 对 252`；`(12,6)：924 对 924`）。

- 对标：729_central_binom → 商式源（546 建表源合流；286 系对峙体例，双边同值锁定）
- 绕行：建表首列留 `1`（546 同式）；商式逐项约分（546 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/730_duel_central/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/730_duel_central/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/730_duel_central/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/730_duel_central/main.sla -o /tmp/basic_730 && /tmp/basic_730
# 期望输出：252,252,924,924
```
