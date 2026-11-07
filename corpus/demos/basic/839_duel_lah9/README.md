# 839 duel_lah9

Lah 续高位对峙：建表 vs 商式（`(9,2)=1451520` 双锁，`(9,3)=1693440` 双锁）。

- 对标：836_lah9 → 建表源（商式版互补；840 做内格对峙）
- 绕行：商式逐项精确整除（`8·362880/2=1451520`，中间积值域内，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/839_duel_lah9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/839_duel_lah9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/839_duel_lah9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/839_duel_lah9/main.sla -o /tmp/basic_839 && /tmp/basic_839
# 期望输出：1451520,1451520,1693440,1693440
```
