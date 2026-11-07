# 630 duel_motz_high

Motzkin 高位对峙：迭代 vs 递归（`n=5：21 对 21`；`n=6：51 对 51`）。

- 对标：628_motz_high → 高位源互补（629 对峙体例上移；双边同值锁定）
- 绕行：`rmoz(6)` 递归扇出两后端秒级（探针：sab 5.6s / sa 8.6s 含全组断言，无超时风险）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/630_duel_motz_high/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/630_duel_motz_high/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/630_duel_motz_high/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/630_duel_motz_high/main.sla -o /tmp/basic_630 && /tmp/basic_630
# 期望输出：21,21,51,51
```
