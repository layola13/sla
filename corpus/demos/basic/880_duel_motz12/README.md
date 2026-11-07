# 880 duel_motz12

Motzkin 续高位四对峙：迭代 vs 直和（`n=11：5798` 双锁，`n=12：15511` 双锁）。

- 对标：879_motz12 × 878_duel_motz11 → 高位源与对峙体例合流（750 体例续高位）
- 绕行：无（双实现互锁，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/880_duel_motz12/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/880_duel_motz12/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/880_duel_motz12/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/880_duel_motz12/main.sla -o /tmp/basic_880 && /tmp/basic_880
# 期望输出：5798,5798,15511,15511
```
