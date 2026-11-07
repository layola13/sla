# 719 duel_nar7

Narayana 行和对峙：行和 vs Catalan 商式（`n=6：132 对 132`；`n=7：429 对 429`）。

- 对标：717_nar_row7 × 351_catalan → 行源与商式源合流（599 体例续高位；双边同值锁定）
- 绕行：商式 `C(14,7)/8=429` 大值精确整除（596 口径；探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/719_duel_nar7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/719_duel_nar7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/719_duel_nar7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/719_duel_nar7/main.sla -o /tmp/basic_719 && /tmp/basic_719
# 期望输出：132,132,429,429
```
