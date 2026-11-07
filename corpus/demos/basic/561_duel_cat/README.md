# 561 duel_cat

Catalan 对峙：卷积递推 `==` 商式 `B(n)/(n+1)`（`n=5`：`42` 对 `42`；`n=6`：`132` 对 `132`）。

- 对标：556_cat_conv × 351_catalan → 两源单实现见各源（286 系对峙体例）
- 绕行：商式侧复用 351 整除链（`binom_mul` 先取小 `kk`，记 README 防中间膨胀）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/561_duel_cat/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/561_duel_cat/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/561_duel_cat/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/561_duel_cat/main.sla -o /tmp/basic_561 && /tmp/basic_561
# 期望输出：42,42,132,132
```
