# 860 duel_central9

中央二项对峙：乘式直算 vs Pascal 建表（`(18,9)=48620` 双锁，`(20,10)=184756` 双锁）。

- 对标：859_central18 × 546_pascal → 两源单实现见各源（730 体例续高位）
- 绕行：表首列逐行置 1（835 探针纠偏复用；探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/860_duel_central9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/860_duel_central9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/860_duel_central9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/860_duel_central9/main.sla -o /tmp/basic_860 && /tmp/basic_860
# 期望输出：48620,48620,184756,184756
```
